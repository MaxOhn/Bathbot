use std::mem;
use std::sync::Arc;
use std::time::Duration;

use bathbot_util::{
    constants::{GENERAL_ISSUE, ORDR_ISSUE},
    EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use futures::future::{ready, BoxFuture};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component,
    },
    guild::Permissions,
    id::{
        marker::{ChannelMarker, MessageMarker, UserMarker},
        Id,
    },
};

pub use self::{recent_score::RecentScoreEdit, top_score::TopScoreEdit};
use crate::{
    active::ComponentResult,
    commands::osu::{OngoingRender, RenderStatus, RenderStatusInner, RENDERER_NAME},
    core::{buckets::BucketName, commands::checks::check_ratelimit},
    manager::{OwnedReplayScore, ReplayScore},
    util::{interaction::InteractionComponent, Authored, Emote, MessageExt},
};
use crate::{
    active::{BuildPage, IActiveMessage},
    core::Context,
};

mod recent_score;
mod top_score;

pub struct EditOnTimeout {
    inner: EditOnTimeoutInner,
    kind: EditOnTimeoutKind,
}

impl IActiveMessage for EditOnTimeout {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(ready(self.inner.build_page()))
    }

    fn build_components(&self) -> Vec<Component> {
        self.kind.build_components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.async_handle_component(ctx, component))
    }

    fn on_timeout<'a>(
        &'a mut self,
        ctx: &'a Context,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> BoxFuture<'a, Result<()>> {
        match self.inner {
            EditOnTimeoutInner::Edit { ref mut edited, .. } => {
                let edited = mem::take(edited);

                let mut builder = MessageBuilder::new()
                    .embed(edited.embed)
                    .components(Vec::new());

                if let Some(ref content) = edited.content {
                    builder = builder.content(content.as_ref());
                }

                match (msg, channel).update(ctx, builder, None) {
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
            EditOnTimeoutInner::Stay(_) => Box::pin(self.kind.on_timeout(ctx, msg, channel)),
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
        ctx: Arc<Context>,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        match &mut self.kind {
            EditOnTimeoutKind::RecentScore(recent_score) => match component.data.custom_id.as_str()
            {
                "miss_analyzer" => {
                    let Some(score_id) = recent_score.take_miss_analyzer() else {
                        return ComponentResult::Err(eyre!(
                            "Unexpected miss analyzer component for recent score"
                        ));
                    };

                    handle_miss_analyzer_button(&ctx, component, score_id).await
                }
                "render" => {
                    let (Some(score_id), score_opt) = recent_score.borrow_mut_render() else {
                        return ComponentResult::Err(eyre!(
                            "Unexpected render component for recent score"
                        ));
                    };

                    let owner = match component.user_id() {
                        Ok(user_id) => user_id,
                        Err(err) => return ComponentResult::Err(err),
                    };

                    if let Some(cooldown) = check_ratelimit(&ctx, owner, BucketName::Render).await {
                        let content = format!(
                            "Rendering is on cooldown for you <@{owner}>, try again in {cooldown} seconds"
                        );

                        let embed = EmbedBuilder::new().description(content).color_red();
                        let builder = MessageBuilder::new().embed(embed);

                        let reply_fut =
                            component
                                .message
                                .reply(&ctx, builder, component.permissions);

                        return match reply_fut.await {
                            Ok(_) => ComponentResult::BuildPage,
                            Err(err) => {
                                let wrap = "Failed to reply for render cooldown error";

                                ComponentResult::Err(Report::new(err).wrap_err(wrap))
                            }
                        };
                    }

                    let Some(score) = score_opt.take() else {
                        return ComponentResult::Err(eyre!("Missing replay score"));
                    };

                    let orig = (component.message.id, component.message.channel_id);
                    let permissions = component.permissions;

                    tokio::spawn(handle_render_button(
                        ctx,
                        orig,
                        permissions,
                        score_id,
                        score,
                        owner,
                    ));

                    ComponentResult::BuildPage
                }
                other => ComponentResult::Err(eyre!("Unknown recent score component `{other}`")),
            },
            EditOnTimeoutKind::TopScore(_) => {
                ComponentResult::Err(eyre!("Unexpected component on single top score"))
            }
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
            Self::RecentScore(recent_score) => {
                let mut components = Vec::new();

                if recent_score.with_miss_analyzer() {
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

                if recent_score.with_render() {
                    let render = Button {
                        custom_id: Some("render".to_owned()),
                        disabled: false,
                        emoji: Some(Emote::Ordr.reaction_type()),
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
            Self::TopScore(_) => Vec::new(),
        }
    }

    fn until_timeout(&self) -> Option<Duration> {
        match self {
            Self::RecentScore(recent_score) => (recent_score.with_miss_analyzer()
                || recent_score.with_render())
            .then_some(Duration::from_secs(45)),
            Self::TopScore(_) => None,
        }
    }

    async fn on_timeout(
        &self,
        ctx: &Context,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> Result<()> {
        match self {
            Self::RecentScore(recent_score)
                if recent_score.with_miss_analyzer() || recent_score.with_render() =>
            {
                let builder = MessageBuilder::new().components(Vec::new());

                match (msg, channel).update(ctx, builder, None) {
                    Some(update_fut) => {
                        update_fut
                            .await
                            .wrap_err("Failed to remove recent score components")?;

                        Ok(())
                    }
                    None => bail!("Lacking permission to update message on timeout"),
                }
            }
            Self::RecentScore(_) => Ok(()),
            Self::TopScore(_) => Ok(()),
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
    ctx: &Context,
    component: &InteractionComponent,
    score_id: u64,
) -> ComponentResult {
    let Some(guild) = component.guild_id.map(Id::get) else {
        return ComponentResult::Err(
            eyre!("Missing guild id for miss analyzer button")
        );
    };

    let channel = component.channel_id.get();
    let msg = component.message.id.get();

    debug!(
        score_id,
        msg, channel, guild, "Sending message to miss analyzer",
    );

    let res_fut = ctx
        .client()
        .miss_analyzer_score_response(guild, channel, msg, score_id);

    if let Err(err) = res_fut.await {
        warn!(?err, "Failed to send miss analyzer response");
    }

    ComponentResult::BuildPage
}

async fn handle_render_button(
    ctx: Arc<Context>,
    orig: (Id<MessageMarker>, Id<ChannelMarker>),
    permissions: Option<Permissions>,
    score_id: u64,
    score: OwnedReplayScore,
    owner: Id<UserMarker>,
) {
    // Check if the score id has already been rendered
    match ctx.replay().get_video_url(score_id).await {
        Ok(Some(video_url)) => {
            let builder = MessageBuilder::new().content(video_url.as_ref());

            if let Err(err) = orig.reply(&ctx, builder, permissions).await {
                error!(?err, "Failed to reply with cached video url");
            }

            return;
        }
        Ok(None) => {}
        Err(err) => warn!(?err),
    }

    let mut status = RenderStatus::new_preparing_replay();
    let score = ReplayScore::Owned(score);

    let msg = match orig.reply(&ctx, status.as_message(), permissions).await {
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

    if let Some(update_fut) = msg.update(&ctx, status.as_message(), permissions) {
        let _ = update_fut.await;
    }

    let replay_manager = ctx.replay();
    let replay_fut = replay_manager.get(Some(score_id), &score);
    let settings_fut = replay_manager.get_settings(owner);

    let (replay_res, settings_res) = tokio::join!(replay_fut, settings_fut);

    let replay = match replay_res {
        Ok(Some(replay)) => replay,
        Ok(None) => {
            let content = "Looks like the replay for that score is not available";

            let embed = EmbedBuilder::new().color_red().description(content);
            let builder = MessageBuilder::new().embed(embed);

            return match msg.update(&ctx, builder, permissions) {
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

            if let Some(update_fut) = msg.update(&ctx, builder, permissions) {
                let _ = update_fut.await;
            }

            return error!(?err, "Failed to get replay");
        }
    };

    let (skin, settings) = match settings_res {
        Ok(tuple) => tuple,
        Err(err) => {
            let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
            let builder = MessageBuilder::new().embed(embed);

            if let Some(update_fut) = msg.update(&ctx, builder, permissions) {
                let _ = update_fut.await;
            }

            return error!(?err);
        }
    };

    status.set(RenderStatusInner::CommissioningRender);

    if let Some(update_fut) = msg.update(&ctx, status.as_message(), permissions) {
        let _ = update_fut.await;
    }

    let render_fut = ctx
        .ordr()
        .client()
        .render_with_replay_file(&replay, RENDERER_NAME, &skin)
        .options(&settings);

    let render = match render_fut.await {
        Ok(render) => render,
        Err(err) => {
            let embed = EmbedBuilder::new().color_red().description(ORDR_ISSUE);
            let builder = MessageBuilder::new().embed(embed);

            if let Some(update_fut) = msg.update(&ctx, builder, permissions) {
                let _ = update_fut.await;
            }

            return error!(?err, "Failed to commission render");
        }
    };

    let ongoing_fut = OngoingRender::new(
        Arc::clone(&ctx),
        render.render_id,
        (msg, permissions),
        status,
        Some(score_id),
    );

    ongoing_fut.await.await_render_url().await;
}
