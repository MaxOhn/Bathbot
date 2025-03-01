use std::{fmt::Write, future::ready, mem};

use bathbot_util::{
    EmbedBuilder, MessageBuilder,
    constants::{GENERAL_ISSUE, ORDR_ISSUE, OSU_API_ISSUE},
};
use eyre::{Report, Result, WrapErr};
use futures::future::BoxFuture;
use rosu_render::{ClientError as OrdrError, client::error::ApiError as OrdrApiError};
use rosu_v2::error::OsuError;
use twilight_model::{
    channel::message::{
        Component,
        component::{ActionRow, Button, ButtonStyle},
    },
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage, response::ActiveResponse},
    commands::osu::{OngoingRender, RENDERER_NAME, RenderStatus, RenderStatusInner},
    core::{Context, buckets::BucketName},
    manager::ReplayError,
    util::{Authored, ComponentExt, MessageExt, interaction::InteractionComponent},
};

pub struct CachedRender {
    score_id: u64,
    video_url: Box<str>,
    msg_owner: Id<UserMarker>,
    done: bool,
}

impl CachedRender {
    pub fn new(score_id: u64, video_url: Box<str>, msg_owner: Id<UserMarker>) -> Self {
        Self {
            score_id,
            video_url,
            msg_owner,
            done: false,
        }
    }

    async fn send_link(&mut self, component: &mut InteractionComponent) -> Result<()> {
        // Anyone, not only the msg_owner, is allowed to use this

        let mut video_url = mem::take(&mut self.video_url).into_string();
        let _ = write!(video_url, " <@{}>", self.msg_owner);

        let builder = MessageBuilder::new()
            .content(video_url)
            .embed(None)
            .components(Vec::new());

        if let Err(err) = component.callback(builder).await {
            return Err(Report::new(err).wrap_err("Failed to callback component"));
        }

        self.done = true;

        Ok(())
    }

    async fn render_anyway(&mut self, component: &mut InteractionComponent) -> Result<()> {
        let owner = component.user_id()?;

        if let Some(cooldown) = Context::check_ratelimit(owner, BucketName::Render) {
            let content = format!(
                "Rendering is on cooldown for you <@{owner}>, try again in {cooldown} seconds"
            );

            let embed = EmbedBuilder::new().description(content).color_red();
            let builder = MessageBuilder::new().embed(embed);

            return component
                .message
                .reply(builder, component.permissions)
                .await
                .map(|_| ())
                .wrap_err("Failed to reply for render cooldown error");
        }

        let mut status = RenderStatus::new_preparing_replay();
        let builder = status.as_message().components(Vec::new());
        component.callback(builder).await?;
        self.done = true;

        let replay_manager = Context::replay();
        let replay_fut = replay_manager.get_replay(self.score_id);
        let settings_fut = replay_manager.get_settings(owner);

        let (replay_res, settings_res) = tokio::join!(replay_fut, settings_fut);

        let replay = match replay_res {
            Ok(Some(replay)) => replay,
            Ok(None) => {
                let embed = EmbedBuilder::new()
                    .color_red()
                    .description("Looks like the replay for that score is not available");

                let builder = MessageBuilder::new().embed(embed);
                component.update(builder).await?;

                return Ok(());
            }
            Err(err) => {
                let (content, err) = match err {
                    ReplayError::AlreadyRequestedCheck(err) => (
                        GENERAL_ISSUE,
                        Some(err.wrap_err(ReplayError::ALREADY_REQUESTED_TEXT)),
                    ),
                    ReplayError::Osu(OsuError::NotFound) => ("Found no score with that id", None),
                    ReplayError::Osu(err) => (
                        OSU_API_ISSUE,
                        Some(Report::new(err).wrap_err("Failed to get replay")),
                    ),
                };

                let embed = EmbedBuilder::new().color_red().description(content);
                let builder = MessageBuilder::new().embed(embed);
                let _ = component.update(builder).await;

                return match err {
                    Some(err) => Err(err),
                    None => return Ok(()),
                };
            }
        };

        let settings = match settings_res {
            Ok(settings) => settings,
            Err(err) => {
                let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
                let builder = MessageBuilder::new().embed(embed);
                let _ = component.update(builder).await;

                return Err(err);
            }
        };

        // Just a status update, no need to propagate an error
        status.set(RenderStatusInner::CommissioningRender);
        let _ = component.update(status.as_message()).await;

        let allow_custom_skins = match component.guild_id {
            Some(guild_id) => {
                Context::guild_config()
                    .peek(guild_id, |config| config.allow_custom_skins.unwrap_or(true))
                    .await
            }
            None => true,
        };

        let skin = settings.skin(allow_custom_skins);

        debug!(
            score_id = self.score_id,
            discord = owner.get(),
            "Commissioning render"
        );

        let render_fut = Context::ordr()
            .client()
            .render_with_replay_file(&replay, RENDERER_NAME, &skin.skin)
            .options(settings.options());

        let render = match render_fut.await {
            Ok(render) => render,
            Err(err) => {
                let (content, err) = match err {
                    OrdrError::Response {
                        error:
                            OrdrApiError {
                                code: Some(code), ..
                            },
                        ..
                    } => (
                        format!("Error code {int} from o!rdr: {code}", int = code.to_u8()),
                        None,
                    ),
                    err => (ORDR_ISSUE.to_owned(), Some(err)),
                };

                let embed = EmbedBuilder::new().color_red().description(content);
                let builder = MessageBuilder::new().embed(embed);
                let _ = component.update(builder).await;

                return match err {
                    Some(err) => Err(Report::new(err).wrap_err("Failed to commission render")),
                    None => return Ok(()),
                };
            }
        };

        let ongoing_fut = OngoingRender::new(
            render.render_id,
            &*component,
            status,
            Some(self.score_id),
            owner,
        );

        tokio::spawn(ongoing_fut.await.await_render_url());

        Ok(())
    }

    async fn async_handle_component(
        &mut self,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let res = match component.data.custom_id.as_str() {
            "send_link" => self.send_link(component).await,
            "render_anyway" => self.render_anyway(component).await,
            other => Err(eyre!("Unknown cached render component `{other}`")),
        };

        match res {
            Ok(_) => ComponentResult::Ignore,
            Err(err) => ComponentResult::Err(err),
        }
    }
}

impl IActiveMessage for CachedRender {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let description =
            "Do you want to save time and send the video link here or re-record this score?";

        let embed = EmbedBuilder::new()
            .title("This score has already been recorded")
            .description(description);

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        if self.done {
            return Vec::new();
        }

        let send_link = Button {
            custom_id: Some("send_link".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Send link".to_owned()),
            style: ButtonStyle::Success,
            url: None,
            sku_id: None,
        };

        let render_anyway = Button {
            custom_id: Some("render_anyway".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Render anyways".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
            sku_id: None,
        };

        let components = vec![
            Component::Button(send_link),
            Component::Button(render_anyway),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.async_handle_component(component))
    }

    fn on_timeout(&mut self, response: ActiveResponse) -> BoxFuture<'_, Result<()>> {
        if self.done {
            return Box::pin(ready(Ok(())));
        }

        let mut video_url = mem::take(&mut self.video_url).into_string();
        let _ = write!(video_url, " <@{}>", self.msg_owner);

        let builder = MessageBuilder::new()
            .content(video_url)
            .embed(None)
            .components(Vec::new());

        let Some(fut) = response.update(builder) else {
            return Box::pin(ready(Err(eyre!(
                "Lacking permissions to handle cached render timeout"
            ))));
        };

        let fut = async {
            fut.await
                .map(|_| ())
                .map_err(|err| Report::new(err).wrap_err("Failed to callback component"))
        };

        Box::pin(fut)
    }
}
