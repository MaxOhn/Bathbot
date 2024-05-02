use std::{mem, time::Duration};

use bathbot_util::{
    constants::{GENERAL_ISSUE, ORDR_ISSUE},
    EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use futures::future::{ready, BoxFuture};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component, ReactionType,
    },
    guild::Permissions,
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};

pub use self::{recent_score::RecentScoreEdit, top_score::TopScoreEdit};
use super::render::CachedRender;
use crate::{
    active::{ActiveMessages, BuildPage, ComponentResult, IActiveMessage},
    commands::osu::{OngoingRender, RenderStatus, RenderStatusInner, RENDERER_NAME},
    core::{buckets::BucketName, Context},
    manager::{OwnedReplayScore, ReplayScore},
    util::{interaction::InteractionComponent, Authored, Emote, MessageExt},
};

mod recent_score;
mod top_score;

pub struct EditOnTimeout {
    inner: EditOnTimeoutInner,
    kind: EditOnTimeoutKind,
}

impl IActiveMessage for EditOnTimeout {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(ready(self.inner.build_page()))
    }

    fn build_components(&self) -> Vec<Component> {
        self.kind.build_components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.async_handle_component(component))
    }

    fn on_timeout(
        &mut self,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> BoxFuture<'_, Result<()>> {
        match self.inner {
            EditOnTimeoutInner::Edit { ref mut edited, .. } => {
                let edited = mem::take(edited);

                let mut builder = MessageBuilder::new()
                    .embed(edited.embed)
                    .components(Vec::new());

                if let Some(ref content) = edited.content {
                    builder = builder.content(content.as_ref());
                }

                match (msg, channel).update(builder, None) {
                    Some(update_fut) => {
                        let fut = async {
                            update_fut
                                .await
                                .map(|_| ())
                                .wrap_err("Failed to edit on timeout")
                        };

                        Box::pin(fut)
                    }
                    None => Box::pin(ready(Err(eyre!("Lacking permission to edit on timeout")))),
                }
            }
            EditOnTimeoutInner::Stay(_) => Box::pin(self.kind.on_timeout(msg, channel)),
        }
    }

    fn until_timeout(&self) -> Option<Duration> {
        match self.inner {
            EditOnTimeoutInner::Edit { .. } => Some(Duration::from_secs(45)),
            EditOnTimeoutInner::Stay(_) => self.kind.until_timeout(),
        }
    }
}

impl EditOnTimeout {
    fn new_stay(build: BuildPage, kind: impl Into<EditOnTimeoutKind>) -> Self {
        Self {
            inner: EditOnTimeoutInner::Stay(build),
            kind: kind.into(),
        }
    }

    fn new_edit(initial: BuildPage, edited: BuildPage, kind: impl Into<EditOnTimeoutKind>) -> Self {
        Self {
            inner: EditOnTimeoutInner::Edit {
                initial,
                edited: Box::new(edited),
            },
            kind: kind.into(),
        }
    }

    async fn async_handle_component(
        &mut self,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let button_data = match &mut self.kind {
            EditOnTimeoutKind::RecentScore(RecentScoreEdit { button_data })
            | EditOnTimeoutKind::TopScore(TopScoreEdit { button_data }) => button_data,
        };

        match component.data.custom_id.as_str() {
            "miss_analyzer" => {
                let Some(score_id) = button_data.take_miss_analyzer() else {
                    return ComponentResult::Err(eyre!(
                        "Unexpected miss analyzer component for recent score"
                    ));
                };

                handle_miss_analyzer_button(component, score_id).await
            }
            "render" => {
                let (Some(score_id), score_opt) = button_data.borrow_mut_render() else {
                    return ComponentResult::Err(eyre!(
                        "Unexpected render component for recent score"
                    ));
                };

                let owner = match component.user_id() {
                    Ok(user_id) => user_id,
                    Err(err) => return ComponentResult::Err(err),
                };

                let Some(score) = score_opt.take() else {
                    return ComponentResult::Err(eyre!("Missing replay score"));
                };

                // Check if the score id has already been rendered
                match Context::replay().get_video_url(score_id).await {
                    Ok(Some(video_url)) => {
                        let channel_id = component.message.channel_id;

                        // Spawn in new task so that we're sure to callback the component in time
                        tokio::spawn(async move {
                            let cached = CachedRender::new(score_id, Some(score), video_url, owner);
                            let begin_fut = ActiveMessages::builder(cached).begin(channel_id);

                            if let Err(err) = begin_fut.await {
                                error!(?err, "Failed to begin cached render message");
                            }
                        });

                        return ComponentResult::BuildPage;
                    }
                    Ok(None) => {}
                    Err(err) => warn!(?err),
                }

                if let Some(cooldown) = Context::check_ratelimit(owner, BucketName::Render) {
                    // Put the score back so that the button can still be used
                    *score_opt = Some(score);

                    let content = format!(
                        "Rendering is on cooldown for you <@{owner}>, try again in {cooldown} seconds"
                    );

                    let embed = EmbedBuilder::new().description(content).color_red();
                    let builder = MessageBuilder::new().embed(embed);

                    let reply_fut = component.message.reply(builder, component.permissions);

                    return match reply_fut.await {
                        Ok(_) => ComponentResult::BuildPage,
                        Err(err) => {
                            let wrap = "Failed to reply for render cooldown error";

                            ComponentResult::Err(Report::new(err).wrap_err(wrap))
                        }
                    };
                }

                let orig = (component.message.id, component.message.channel_id);
                let permissions = component.permissions;

                tokio::spawn(handle_render_button(
                    orig,
                    permissions,
                    score_id,
                    score,
                    owner,
                    component.guild_id,
                ));

                ComponentResult::BuildPage
            }
            other => ComponentResult::Err(eyre!("Unknown EditOnTimeout component `{other}`")),
        }
    }
}

enum EditOnTimeoutKind {
    RecentScore(RecentScoreEdit),
    TopScore(TopScoreEdit),
}

impl EditOnTimeoutKind {
    fn build_components(&self) -> Vec<Component> {
        match self {
            Self::RecentScore(RecentScoreEdit { button_data })
            | Self::TopScore(TopScoreEdit { button_data }) => {
                let mut components = Vec::new();

                if button_data.with_miss_analyzer() {
                    let miss_analyzer = Button {
                        custom_id: Some("miss_analyzer".to_owned()),
                        disabled: false,
                        emoji: Some(Emote::Miss.reaction_type()),
                        label: Some("Miss analyzer".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    };

                    components.push(Component::Button(miss_analyzer));
                }

                if button_data.with_render() {
                    let render = Button {
                        custom_id: Some("render".to_owned()),
                        disabled: false,
                        emoji: Some(ReactionType::Unicode {
                            name: "🎥".to_owned(),
                        }),
                        label: Some("Render".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    };

                    components.push(Component::Button(render));
                }

                if !components.is_empty() {
                    components = vec![Component::ActionRow(ActionRow { components })]
                }

                components
            }
        }
    }

    fn until_timeout(&self) -> Option<Duration> {
        match self {
            Self::RecentScore(RecentScoreEdit { button_data })
            | Self::TopScore(TopScoreEdit { button_data }) => (button_data.with_miss_analyzer()
                || button_data.with_render())
            .then_some(Duration::from_secs(45)),
        }
    }

    async fn on_timeout(&self, msg: Id<MessageMarker>, channel: Id<ChannelMarker>) -> Result<()> {
        match self {
            Self::RecentScore(RecentScoreEdit { button_data })
            | Self::TopScore(TopScoreEdit { button_data })
                if button_data.with_miss_analyzer() || button_data.with_render() =>
            {
                let builder = MessageBuilder::new().components(Vec::new());

                match (msg, channel).update(builder, None) {
                    Some(update_fut) => {
                        update_fut
                            .await
                            .wrap_err("Failed to remove recent score components")?;

                        Ok(())
                    }
                    None => bail!("Lacking permission to update message on timeout"),
                }
            }
            Self::RecentScore(_) | Self::TopScore(_) => Ok(()),
        }
    }
}

enum EditOnTimeoutInner {
    Stay(BuildPage),
    Edit {
        initial: BuildPage,
        edited: Box<BuildPage>,
    },
}

impl EditOnTimeoutInner {
    fn build_page(&mut self) -> Result<BuildPage> {
        match self {
            EditOnTimeoutInner::Stay(build) => Ok(build.to_owned()),
            EditOnTimeoutInner::Edit { initial, .. } => Ok(initial.to_owned()),
        }
    }
}

async fn handle_miss_analyzer_button(
    component: &InteractionComponent,
    score_id: u64,
) -> ComponentResult {
    let Some(guild) = component.guild_id.map(Id::get) else {
        return ComponentResult::Err(eyre!("Missing guild id for miss analyzer button"));
    };

    let channel = component.channel_id.get();
    let msg = component.message.id.get();

    debug!(
        score_id,
        msg, channel, guild, "Sending message to miss analyzer",
    );

    let res_fut = Context::client().miss_analyzer_score_response(guild, channel, msg, score_id);

    if let Err(err) = res_fut.await {
        warn!(?err, "Failed to send miss analyzer response");
    }

    ComponentResult::BuildPage
}

async fn handle_render_button(
    orig: (Id<MessageMarker>, Id<ChannelMarker>),
    permissions: Option<Permissions>,
    score_id: u64,
    score: OwnedReplayScore,
    owner: Id<UserMarker>,
    guild: Option<Id<GuildMarker>>,
) {
    let mut status = RenderStatus::new_preparing_replay();
    let score = ReplayScore::from(score);

    let msg = match orig.reply(status.as_message(), permissions).await {
        Ok(response) => match response.model().await {
            Ok(msg) => msg,
            Err(err) => {
                return error!(
                    ?err,
                    "Failed to deserialize reply after render button click"
                )
            }
        },
        Err(err) => return error!(?err, "Failed to reply after render button click"),
    };

    status.set(RenderStatusInner::PreparingReplay);

    if let Some(update_fut) = msg.update(status.as_message(), permissions) {
        let _ = update_fut.await;
    }

    let replay_manager = Context::replay();
    let replay_fut = replay_manager.get_replay(score_id, &score);
    let settings_fut = replay_manager.get_settings(owner);

    let (replay_res, settings_res) = tokio::join!(replay_fut, settings_fut);

    let replay = match replay_res {
        Ok(Some(replay)) => replay,
        Ok(None) => {
            let content = "Looks like the replay for that score is not available";

            let embed = EmbedBuilder::new().color_red().description(content);
            let builder = MessageBuilder::new().embed(embed);

            return match msg.update(builder, permissions) {
                Some(update_fut) => match update_fut.await {
                    Ok(_) => {}
                    Err(err) => error!(?err, "Failed to update message"),
                },
                None => warn!("Lacking permission to update message on error"),
            };
        }
        Err(err) => {
            let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
            let builder = MessageBuilder::new().embed(embed);

            if let Some(update_fut) = msg.update(builder, permissions) {
                let _ = update_fut.await;
            }

            return error!(?err, "Failed to get replay");
        }
    };

    let settings = match settings_res {
        Ok(settings) => settings,
        Err(err) => {
            let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
            let builder = MessageBuilder::new().embed(embed);

            if let Some(update_fut) = msg.update(builder, permissions) {
                let _ = update_fut.await;
            }

            return error!(?err);
        }
    };

    status.set(RenderStatusInner::CommissioningRender);

    if let Some(update_fut) = msg.update(status.as_message(), permissions) {
        let _ = update_fut.await;
    }

    let allow_custom_skins = match guild {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| config.allow_custom_skins.unwrap_or(true))
                .await
        }
        None => true,
    };

    let skin = settings.skin(allow_custom_skins);

    let render_fut = Context::ordr()
        .expect("ordr unavailable")
        .client()
        .render_with_replay_file(&replay, RENDERER_NAME, &skin.skin)
        .options(settings.options());

    let render = match render_fut.await {
        Ok(render) => render,
        Err(err) => {
            let embed = EmbedBuilder::new().color_red().description(ORDR_ISSUE);
            let builder = MessageBuilder::new().embed(embed);

            if let Some(update_fut) = msg.update(builder, permissions) {
                let _ = update_fut.await;
            }

            return error!(?err, "Failed to commission render");
        }
    };

    let ongoing_fut = OngoingRender::new(
        render.render_id,
        (msg, permissions),
        status,
        Some(score_id),
        owner,
    );

    ongoing_fut.await.await_render_url().await;
}

struct ButtonData {
    score_id: Option<u64>,
    with_miss_analyzer_button: bool,
    replay_score: Option<OwnedReplayScore>,
}

impl ButtonData {
    fn with_miss_analyzer(&self) -> bool {
        self.score_id.is_some() && self.with_miss_analyzer_button
    }

    fn take_miss_analyzer(&mut self) -> Option<u64> {
        self.score_id
            .filter(|_| mem::replace(&mut self.with_miss_analyzer_button, false))
    }

    fn with_render(&self) -> bool {
        self.score_id.is_some() && self.replay_score.is_some()
    }

    fn borrow_mut_render(&mut self) -> (Option<u64>, &mut Option<OwnedReplayScore>) {
        (self.score_id, &mut self.replay_score)
    }
}
