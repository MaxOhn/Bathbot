use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
    time::Duration,
};

use bathbot_model::HlVersion;
use bathbot_util::MessageBuilder;
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use tokio::sync::oneshot::Receiver;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        embed::{EmbedField, EmbedFooter},
        Component, ReactionType,
    },
    id::{
        marker::{ChannelMarker, MessageMarker, UserMarker},
        Id,
    },
};

use self::state::{ButtonState, HigherLowerState};
use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    core::Context,
    util::{interaction::InteractionComponent, Authored, ComponentExt, Emote, MessageExt},
};

mod farm_maps;
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
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page(ctx))
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

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err).boxed(),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore.boxed();
        }

        match component.data.custom_id.as_str() {
            "higher_button" => Box::pin(self.handle_higherlower(ctx, component, HlGuess::Higher)),
            "lower_button" => Box::pin(self.handle_higherlower(ctx, component, HlGuess::Lower)),
            "next_higherlower" => Box::pin(self.handle_next(ctx, component)),
            "try_again_button" => Box::pin(self.handle_try_again(ctx, component)),
            other => {
                warn!(name = %other, ?component, "Unknown higherlower component");

                ComponentResult::Ignore.boxed()
            }
        }
    }

    fn on_timeout<'a>(
        &'a mut self,
        ctx: &'a Context,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(self.async_on_timeout(ctx, msg, channel))
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
    pub async fn new_score_pp(
        ctx: &Context,
        mode: GameMode,
        msg_owner: Id<UserMarker>,
    ) -> Result<Self> {
        let game_fut = HigherLowerState::start_score_pp(ctx, mode);
        let highscore_fut = ctx
            .games()
            .higherlower_highscore(msg_owner, HlVersion::ScorePp);

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

    pub async fn new_farm_maps(ctx: &Context, msg_owner: Id<UserMarker>) -> Result<Self> {
        let entries_fut = ctx.redis().osutracker_counts();
        let highscore_fut = ctx
            .games()
            .higherlower_highscore(msg_owner, HlVersion::FarmMaps);

        let (entries_res, highscore_res) = tokio::join!(entries_fut, highscore_fut);
        let highscore = highscore_res.wrap_err("Failed to get highscore from database")?;

        let (state, rx) = HigherLowerState::start_farm_maps(ctx, entries_res?)
            .await
            .wrap_err("Failed to create farm maps game state")?;

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

    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
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
                    .next(ctx, self.current_score)
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

                let value = if self.new_highscore(&ctx).await? {
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

    async fn async_on_timeout(
        &mut self,
        ctx: &Context,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> Result<()> {
        let builder = MessageBuilder::new().components(self.disabled_buttons());

        let update_res = match (msg, channel).update(ctx, &builder, None) {
            Some(update_fut) => update_fut.await,
            None => return Err(eyre!("Lacking permission to disable components on timeout")),
        };

        self.new_highscore(ctx)
            .await
            .wrap_err("Failed to update highscore on timeout")?;

        update_res.wrap_err("Failed to disable components")?;

        Ok(())
    }

    async fn handle_higherlower(
        &mut self,
        ctx: &Context,
        component: &mut InteractionComponent,
        guess: HlGuess,
    ) -> ComponentResult {
        self.revealed = true;

        let Some(embed) = component.message.embeds.pop() else {
            return ComponentResult::Err(eyre!("Missing higherlower embed"));
        };

        let image = embed.image.map(|image| image.url.into_boxed_str());

        if self.state.check_guess(guess) {
            if let Err(err) = component.defer(ctx).await {
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

    async fn handle_next(
        &mut self,
        ctx: &Context,
        component: &InteractionComponent,
    ) -> ComponentResult {
        if let Err(err) = component.defer(ctx).await {
            warn!(?err, "Failed to defer next button");
        }

        self.revealed = false;
        self.buttons = ButtonState::HigherLower;

        ComponentResult::BuildPage
    }

    async fn handle_try_again(
        &mut self,
        ctx: &Context,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let Some(mut embed) = component.message.embeds.pop() else {
            return ComponentResult::Err(eyre!("Missing embed in higherlower message"));
        };

        let footer = EmbedFooter {
            icon_url: None,
            proxy_icon_url: None,
            text: "Preparing game, give me a moment...".to_owned(),
        };

        embed.footer = Some(footer);

        let builder = MessageBuilder::new()
            .embed(embed)
            .components(self.disabled_buttons());

        if let Err(err) = component.callback(ctx, builder).await {
            warn!(?err, "Failed to callback try again button");
        }

        let (state, rx) = match self.state.restart(ctx).await {
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

    async fn new_highscore(&self, ctx: &Context) -> Result<bool> {
        ctx.games()
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
        };

        let lower = Button {
            custom_id: Some("lower_button".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::HigherLower),
            emoji: None,
            label: Some("Lower".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let next = Button {
            custom_id: Some("next_higherlower".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::Next { .. }),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: Some("Next".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
        };

        let retry = Button {
            custom_id: Some("try_again_button".to_owned()),
            disabled: !matches!(self.buttons, ButtonState::TryAgain { .. }),
            emoji: Some(ReactionType::Unicode {
                name: "🔁".to_owned(),
            }),
            label: Some("Try Again".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
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
