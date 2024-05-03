use std::{fmt::Write, future::ready, mem};

use bathbot_util::{
    constants::{GENERAL_ISSUE, ORDR_ISSUE, OSU_API_ISSUE},
    EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{response::ActiveResponse, BuildPage, ComponentResult, IActiveMessage},
    commands::osu::{OngoingRender, RenderStatus, RenderStatusInner, RENDERER_NAME},
    core::{buckets::BucketName, Context},
    manager::{OwnedReplayScore, ReplayScore},
    util::{interaction::InteractionComponent, Authored, ComponentExt, MessageExt},
};

pub struct CachedRender {
    score_id: u64,
    score: Option<OwnedReplayScore>,
    video_url: Box<str>,
    msg_owner: Id<UserMarker>,
    done: bool,
}

impl CachedRender {
    pub fn new(
        score_id: u64,
        score: Option<OwnedReplayScore>,
        video_url: Box<str>,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            score_id,
            score,
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

        let (mut status, (replay_res, settings_res)) = match self.score.take() {
            Some(score) => {
                let status = RenderStatus::new_preparing_replay();
                let builder = status.as_message().components(Vec::new());
                component.callback(builder).await?;
                self.done = true;

                let replay_manager = Context::replay();
                let score = ReplayScore::from(score);
                let replay_fut = replay_manager.get_replay(self.score_id, &score);
                let settings_fut = replay_manager.get_settings(owner);

                (status, tokio::join!(replay_fut, settings_fut))
            }
            None => {
                let mut status = RenderStatus::new_requesting_score();
                let builder = status.as_message().components(Vec::new());
                component.callback(builder).await?;
                self.done = true;

                let score = match Context::osu().score(self.score_id, GameMode::Osu).await {
                    Ok(score) => score,
                    Err(err) => {
                        let embed = EmbedBuilder::new().color_red().description(OSU_API_ISSUE);
                        let builder = MessageBuilder::new().embed(embed);
                        let _ = component.update(builder).await;

                        return Err(Report::new(err).wrap_err("Failed to get score"));
                    }
                };

                let Some(replay_score) = ReplayScore::from_score(&score) else {
                    let content = "Failed to prepare the replay";
                    let embed = EmbedBuilder::new().color_red().description(content);
                    let builder = MessageBuilder::new().embed(embed);
                    component.update(builder).await?;

                    return Ok(());
                };

                let Some(score_id) = score.legacy_score_id else {
                    let content = "Scores on osu!lazer currently cannot be rendered :(";
                    let embed = EmbedBuilder::new().color_red().description(content);
                    let builder = MessageBuilder::new().embed(embed);
                    component.update(builder).await?;

                    return Ok(());
                };

                // Just a status update, no need to propagate an error
                status.set(RenderStatusInner::PreparingReplay);
                let _ = component.update(status.as_message()).await;

                let replay_manager = Context::replay();
                let replay_fut = replay_manager.get_replay(score_id, &replay_score);
                let settings_fut = replay_manager.get_settings(owner);

                (status, tokio::join!(replay_fut, settings_fut))
            }
        };

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
                let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
                let builder = MessageBuilder::new().embed(embed);
                let _ = component.update(builder).await;

                return Err(err.wrap_err("Failed to get replay"));
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
                let _ = component.update(builder).await;

                return Err(Report::new(err).wrap_err("Failed to commission render"));
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
        };

        let render_anyway = Button {
            custom_id: Some("render_anyway".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Render anyways".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
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
