use std::mem;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "twitch")]
use bathbot_model::TwitchVideo;
use bathbot_util::MessageBuilder;
use eyre::{Result, WrapErr};
use futures::future::{ready, BoxFuture};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component,
    },
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

pub use self::{recent_score::RecentScoreEdit, top_score::TopScoreEdit};
use crate::{
    active::{ActiveMessage, ComponentResult},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Emote, MessageExt,
    },
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
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.kind.handle_component(ctx, component))
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

                if let Some(content) = edited.content {
                    builder = builder.content(content);
                }

                match (msg, channel).update(ctx, &builder, None) {
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
            inner: EditOnTimeoutInner::Edit { initial, edited },
            kind: kind.into(),
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
            Self::RecentScore(recent_score) => match recent_score.miss_analyzer_score_id {
                Some(_) => {
                    let miss_analyzer = Button {
                        custom_id: Some("miss_analyzer".to_owned()),
                        disabled: false,
                        emoji: Some(Emote::Miss.reaction_type()),
                        label: Some("Miss analyzer".to_owned()),
                        style: ButtonStyle::Secondary,
                        url: None,
                    };

                    let components = vec![Component::Button(miss_analyzer)];

                    vec![Component::ActionRow(ActionRow { components })]
                }
                None => Vec::new(),
            },
            Self::TopScore(_) => Vec::new(),
        }
    }

    async fn handle_component(
        &mut self,
        ctx: &Context,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        match self {
            Self::RecentScore(recent_score) => {
                let Some(score_id) = recent_score.miss_analyzer_score_id.take() else {
                    return ComponentResult::Err(eyre!(
                        "Unexpected component for recent score without score id"
                    ));
                };

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
            Self::TopScore(_) => {
                ComponentResult::Err(eyre!("Unexpected component on single top score"))
            }
        }
    }

    fn until_timeout(&self) -> Option<Duration> {
        match self {
            Self::RecentScore(recent_score) => recent_score
                .miss_analyzer_score_id
                .map(|_| Duration::from_secs(45)),
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
            Self::RecentScore(recent_score) => match recent_score.miss_analyzer_score_id {
                Some(_) => {
                    let builder = MessageBuilder::new().components(Vec::new());

                    match (msg, channel).update(ctx, &builder, None) {
                        Some(update_fut) => {
                            update_fut
                                .await
                                .wrap_err("Failed to remove recent score components")?;

                            Ok(())
                        }
                        None => bail!("Lacking permission to update message on timeout"),
                    }
                }
                None => Ok(()),
            },
            Self::TopScore(_) => Ok(()),
        }
    }
}

// TODO: slim down EmbedBuilder
#[allow(clippy::large_enum_variant)]
enum EditOnTimeoutInner {
    Stay(BuildPage),
    Edit {
        initial: BuildPage,
        edited: BuildPage,
    },
}

impl EditOnTimeoutInner {
    fn build_page(&self) -> Result<BuildPage> {
        match self {
            EditOnTimeoutInner::Stay(build) => Ok(build.to_owned()),
            EditOnTimeoutInner::Edit { initial, .. } => Ok(initial.to_owned()),
        }
    }
}

// TODO: remove when EmbedBuilder has been slimmed down
impl From<EditOnTimeout> for ActiveMessage {
    fn from(edit_on_timeout: EditOnTimeout) -> Self {
        Self::EditOnTimeout(Box::new(edit_on_timeout))
    }
}

// TODO: remove when EmbedBuilder has been slimmed down
impl IActiveMessage for Box<EditOnTimeout> {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        EditOnTimeout::build_page(self, ctx)
    }

    fn build_components(&self) -> Vec<Component> {
        EditOnTimeout::build_components(self)
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        EditOnTimeout::handle_component(self, ctx, component)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        EditOnTimeout::handle_modal(self, ctx, modal)
    }

    fn on_timeout<'a>(
        &'a mut self,
        ctx: &'a Context,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> BoxFuture<'a, Result<()>> {
        EditOnTimeout::on_timeout(self, ctx, msg, channel)
    }

    fn until_timeout(&self) -> Option<Duration> {
        EditOnTimeout::until_timeout(self)
    }
}
