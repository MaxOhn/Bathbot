use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use bathbot_model::HlVersion;
use bathbot_util::{AuthorBuilder, Authored, EmbedBuilder, MessageBuilder};
use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use tokio::sync::oneshot::Receiver;
use twilight_model::{
    channel::message::{
        Component, EmojiReactionType,
        component::{ActionRow, Button, ButtonStyle},
        embed::EmbedField,
    },
    id::{Id, marker::UserMarker},
};

use self::state::{ButtonState, HigherLowerState};
use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage, response::ActiveResponse},
    core::Context,
    util::{ComponentExt, Emote, interaction::InteractionComponent},
};

mod score_pp;
mod state;

pub struct HigherLowerGame {
    state: HigherLowerState,
    revealed: bool,
    img_url_rx: Option<Receiver<String>>,
    current_score: u32,
    highscore: u32,
    buttons: ButtonState,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for HigherLowerGame {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let mut embed = self.state.to_embed(self.revealed);

        let deferred = match self.buttons {
            ButtonState::HigherLower => {
                let footer = format!(
                    "Current score: {} • Highscore: {}",
                    self.current_score, self.highscore
                );

                embed = embed.footer(footer);

                match self.img_url_rx.take() {
                    Some(rx) => match rx.await {
                        Ok(url) => embed = embed.image(url),
                        Err(err) => warn!(?err, "Failed to receive image url"),
                    },
                    None => warn!("Tried to await image rx after it's already been used"),
                }

                true
            }
            ButtonState::Next {
                ref mut image,
                last_guess,
            } => {
                let footer = format!(
                    "Current score: {} • Highscore: {} • \
                    {last_guess} was correct, press Next to continue",
                    self.current_score, self.highscore
                );

                embed = embed.footer(footer);

                let rx = self
                    .state
                    .next(self.current_score)
                    .await
                    .wrap_err("Failed to get next game")?;

                self.img_url_rx = Some(rx);

                if let Some(image) = image.take() {
                    embed = embed.image(image);
                }

                true
            }
            ButtonState::TryAgain {
                ref mut image,
                last_guess,
            } => {
                if let Some(image) = image.take() {
                    embed = embed.image(image);
                }

                let value = if self.new_highscore().await? {
                    format!(
                        "You achieved a total score of {}, your new personal best :tada:",
                        self.current_score
                    )
                } else {
                    format!(
                        "You achieved a total score of {}, your personal best is {}.",
                        self.current_score, self.highscore,
                    )
                };

                let name = format!("Game Over - {last_guess} was incorrect");

                let field = EmbedField {
                    inline: false,
                    name,
                    value,
                };

                embed.push_field(field);

                false
            }
        };

        Ok(BuildPage::new(embed, deferred))
    }

    fn build_components(&self) -> Vec<Component> {
        let [higher, lower, next, retry] = self.raw_buttons();

        let button_row = ActionRow {
            components: vec![
                Component::Button(higher),
                Component::Button(lower),
                Component::Button(next),
                Component::Button(retry),
            ],
        };

        vec![Component::ActionRow(button_row)]
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        match component.data.custom_id.as_str() {
            "higher_button" => self.handle_higherlower(component, HlGuess::Higher).await,
            "lower_button" => self.handle_higherlower(component, HlGuess::Lower).await,
            "next_higherlower" => self.handle_next(component).await,
            "try_again_button" => self.handle_try_again(component).await,
            other => {
                warn!(name = %other, ?component, "Unknown higherlower component");

                ComponentResult::Ignore
            }
        }
    }

    async fn on_timeout(&mut self, response: ActiveResponse) -> Result<()> {
        let builder = MessageBuilder::new().components(self.disabled_buttons());

        let update_res = match response.update(builder) {
            Some(update_fut) => update_fut.await,
            None => bail!("Lacking permission to disable components on timeout"),
        };

        self.new_highscore()
            .await
            .wrap_err("Failed to update highscore on timeout")?;

        update_res.wrap_err("Failed to disable components")?;

        Ok(())
    }

    fn until_timeout(&self) -> Option<Duration> {
        match self.buttons {
            ButtonState::HigherLower => Some(Duration::from_secs(90)),
            ButtonState::Next { .. } => Some(Duration::from_secs(30)),
            ButtonState::TryAgain { .. } => Some(Duration::from_secs(30)),
        }
    }
}

impl HigherLowerGame {
    pub async fn new_score_pp(mode: GameMode, msg_owner: Id<UserMarker>) -> Result<Self> {
        let game_fut = HigherLowerState::start_score_pp(mode);
        let highscore_fut = Context::games().higherlower_highscore(msg_owner, HlVersion::ScorePp);

        let ((state, rx), highscore) = tokio::try_join!(game_fut, highscore_fut)?;

        Ok(Self {
            state,
            revealed: false,
            img_url_rx: Some(rx),
            current_score: 0,
            highscore,
            buttons: ButtonState::HigherLower,
            msg_owner,
        })
    }

    async fn handle_higherlower(
        &mut self,
        component: &mut InteractionComponent,
        guess: HlGuess,
    ) -> ComponentResult {
        self.revealed = true;

        let Some(embed) = component.message.embeds.pop() else {
            return ComponentResult::Err(eyre!("Missing higherlower embed"));
        };

        let image = embed.image.map(|image| image.url.into_boxed_str());

        if self.state.check_guess(guess) {
            if let Err(err) = component.defer().await {
                warn!(?err, "Failed to defer higherlower button");
            }

            self.current_score += 1;

            self.buttons = ButtonState::Next {
                image,
                last_guess: guess,
            };

            ComponentResult::BuildPage
        } else {
            self.buttons = ButtonState::TryAgain {
                image,
                last_guess: guess,
            };

            ComponentResult::BuildPage
        }
    }

    async fn handle_next(&mut self, component: &InteractionComponent) -> ComponentResult {
        if let Err(err) = component.defer().await {
            warn!(?err, "Failed to defer next button");
        }

        self.revealed = false;
        self.buttons = ButtonState::HigherLower;

        ComponentResult::BuildPage
    }

    async fn handle_try_again(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let Some(embed) = component.message.embeds.pop() else {
            return ComponentResult::Err(eyre!("Missing embed in higherlower message"));
        };

        // Little awkward to go from Embed -> EmbedBuilder but MessageBuilder
        // requires EmbedBuilder
        let mut eb = EmbedBuilder::new()
            .fields(embed.fields)
            .footer("Preparing game, give me a moment...");

        if let Some(author) = embed.author {
            let mut ab = AuthorBuilder::new(author.name);

            if let Some(url) = author.url {
                ab = ab.url(url);
            }

            if let Some(icon_url) = author.icon_url {
                ab = ab.icon_url(icon_url);
            }

            eb = eb.author(ab);
        }

        if let Some(description) = embed.description {
            eb = eb.description(description);
        }

        if let Some(image) = embed.image {
            eb = eb.image(image.url);
        }

        if let Some(thumbnail) = embed.thumbnail {
            eb = eb.thumbnail(thumbnail.url);
        }

        if let Some(timestamp) = embed.timestamp {
            eb = eb.timestamp(OffsetDateTime::from_unix_timestamp(timestamp.as_secs()).unwrap());
        }

        if let Some(title) = embed.title {
            eb = eb.title(title);
        }

        if let Some(url) = embed.url {
            eb = eb.url(url);
        }

        let builder = MessageBuilder::new()
            .embed(eb)
            .components(self.disabled_buttons());

        if let Err(err) = component.callback(builder).await {
            warn!(?err, "Failed to callback try again button");
        }

        let (state, rx) = match self.state.restart().await {
            Ok(tuple) => tuple,
            Err(err) => return ComponentResult::Err(err),
        };

        self.state = state;
        self.img_url_rx = Some(rx);
        self.highscore = self.highscore.max(self.current_score);
        self.current_score = 0;
        self.revealed = false;
        self.buttons = ButtonState::HigherLower;

        ComponentResult::BuildPage
    }

    async fn new_highscore(&self) -> Result<bool> {
        Context::games()
            .upsert_higherlower_score(self.msg_owner, self.state.version(), self.current_score)
            .await
            .wrap_err("Failed to upsert higherlower score")
    }

    fn raw_buttons(&self) -> [Button; 4] {
        let higher = Button {
            custom_id: Some("higher_button".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::HigherLower),
            emoji: None,
            label: Some("Higher".to_owned()),
            style: ButtonStyle::Success,
            url: None,
            sku_id: None,
        };

        let lower = Button {
            custom_id: Some("lower_button".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::HigherLower),
            emoji: None,
            label: Some("Lower".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
            sku_id: None,
        };

        let next = Button {
            custom_id: Some("next_higherlower".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::Next { .. }),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: Some("Next".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let retry = Button {
            custom_id: Some("try_again_button".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::TryAgain { .. }),
            emoji: Some(EmojiReactionType::Unicode {
                name: "🔁".to_owned(),
            }),
            label: Some("Try Again".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        [higher, lower, next, retry]
    }

    fn disabled_buttons(&self) -> Vec<Component> {
        let mut buttons = self.raw_buttons();
        buttons.iter_mut().for_each(|button| button.disabled = true);
        let components = buttons.into_iter().map(Component::Button).collect();

        vec![Component::ActionRow(ActionRow { components })]
    }
}

#[derive(Copy, Clone)]
enum HlGuess {
    Higher,
    Lower,
}

impl Display for HlGuess {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            HlGuess::Higher => f.write_str("Higher"),
            HlGuess::Lower => f.write_str("Lower"),
        }
    }
}
