use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
    time::{Duration, Instant},
};

use bathbot_macros::SlashCommand;
use bathbot_util::{
    constants::{GENERAL_ISSUE, ORDR_ISSUE, OSU_API_ISSUE},
    EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use rosu_render::error::{Error as OrdrError, ErrorCode as OrdrErrorCode};
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::Attachment,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        impls::{RenderSettingsActive, SettingsImport},
        ActiveMessages,
    },
    core::{
        buckets::BucketName,
        commands::{checks::check_ratelimit, OwnedCommandOrigin},
        Context,
    },
    manager::ReplayScore,
    tracking::OrdrReceivers,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub const RENDERER_NAME: &str = "Bathbot";

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "render",
    desc = "Render an osu!standard play via o!rdr",
    help = "Render a play via o!rdr.\n\
    Since [danser](https://github.com/Wieku/danser-go) is being used, \
    only osu!standard is supported."
)]
#[flags(SKIP_DEFER)]
pub enum Render {
    #[command(name = "replay")]
    Replay(RenderReplay),
    #[command(name = "score")]
    Score(RenderScore),
    #[command(name = "settings")]
    Settings(RenderSettings),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "replay", desc = "Render a replay")]
pub struct RenderReplay {
    #[command(desc = "Specify the replay through a .osr file")]
    replay: Attachment,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "score", desc = "Render a score")]
pub struct RenderScore {
    #[command(desc = "Specify the score through its id")]
    score_id: u64,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "settings", desc = "Adjust your o!rdr render settings")]
pub enum RenderSettings {
    #[command(name = "modify")]
    Modify(RenderSettingsModify),
    #[command(name = "import")]
    Import(RenderSettingsImport),
    #[command(name = "copy")]
    Copy(RenderSettingsCopy),
    #[command(name = "default")]
    Default(RenderSettingsDefault),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "modify", desc = "Modify your o!rdr render settings")]
pub struct RenderSettingsModify;

#[derive(CommandModel, CreateCommand)]
#[command(name = "import", desc = "Import your render settings from Yuna bot")]
pub struct RenderSettingsImport;

#[derive(CommandModel, CreateCommand)]
#[command(name = "copy", desc = "Use someone else's render settings as your own")]
pub struct RenderSettingsCopy {
    #[command(desc = "Specify a user to copy render settings from")]
    user: Id<UserMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "default", desc = "Reset your render settings to the default")]
pub struct RenderSettingsDefault;

pub async fn slash_render(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Render::from_interaction(command.input_data())? {
        Render::Replay(args) => render_replay(ctx, command, args).await,
        Render::Score(args) => render_score(ctx, command, args).await,
        Render::Settings(RenderSettings::Modify(_)) => {
            render_settings_modify(ctx, &mut command).await
        }
        Render::Settings(RenderSettings::Import(_)) => {
            render_settings_import(ctx, &mut command).await
        }
        Render::Settings(RenderSettings::Copy(args)) => {
            render_settings_copy(ctx, &mut command, args).await
        }
        Render::Settings(RenderSettings::Default(_)) => {
            render_settings_default(ctx, &mut command).await
        }
    }
}

async fn render_replay(
    ctx: Arc<Context>,
    command: InteractionCommand,
    replay: RenderReplay,
) -> Result<()> {
    let owner = command.user_id()?;

    if let Some(cooldown) = check_ratelimit(&ctx, owner, BucketName::Render).await {
        trace!("Ratelimiting user {owner} on bucket `Render` for {cooldown} seconds");

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let RenderReplay { replay } = replay;

    if !replay.filename.ends_with(".osr") {
        let content = "The attached replay must be a .osr file";
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let status = RenderStatus::new_commisioning_replay();
    command.callback(&ctx, status.as_message(), false).await?;

    let (skin, settings) = match ctx.replay().get_settings(owner).await {
        Ok(tuple) => tuple,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let render_fut = ctx
        .ordr()
        .client()
        .render_with_replay_url(&replay.url, RENDERER_NAME, &skin)
        .options(&settings);

    let render = match render_fut.await {
        Ok(render) => render,
        Err(OrdrError::Response { error, .. }) if error.code == OrdrErrorCode::InvalidGameMode => {
            let content = "I can only render osu!standard scores";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, ORDR_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to commission render"));
        }
    };

    let ongoing = OngoingRender::new(ctx, render.render_id, command, status, None).await;

    tokio::spawn(ongoing.await_render_url());

    Ok(())
}

async fn render_score(
    ctx: Arc<Context>,
    command: InteractionCommand,
    score: RenderScore,
) -> Result<()> {
    let owner = command.user_id()?;

    command.defer(&ctx, false).await?;

    let RenderScore { score_id } = score;

    // Check if the score id has already been rendered
    match ctx.replay().get_video_url(score_id).await {
        Ok(Some(video_url)) => {
            let builder = MessageBuilder::new().content(video_url.as_ref());
            command.update(&ctx, builder).await?;

            return Ok(());
        }
        Ok(None) => {}
        Err(err) => warn!(?err),
    }

    if let Some(cooldown) = check_ratelimit(&ctx, owner, BucketName::Render).await {
        trace!("Ratelimiting user {owner} on bucket `Render` for {cooldown} seconds");

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        command.error(&ctx, content).await?;

        return Ok(());
    }

    let mut status = RenderStatus::new_requesting_score();
    command.update(&ctx, status.as_message()).await?;

    let score = match ctx.osu().score(score_id, GameMode::Osu).await {
        Ok(score) => score,
        Err(OsuError::NotFound) => {
            let content = "Found no osu!standard score with that id";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get score"));
        }
    };

    let replay_score = ReplayScore::from(&score);

    // Just a status update, no need to propagate an error
    status.set(RenderStatusInner::PreparingReplay);
    let _ = command.update(&ctx, status.as_message()).await;

    let replay_manager = ctx.replay();
    let replay_fut = replay_manager.get(score.score_id, &replay_score);
    let settings_fut = replay_manager.get_settings(owner);

    let (replay_res, settings_res) = tokio::join!(replay_fut, settings_fut);

    let replay = match replay_res {
        Ok(Some(replay)) => replay,
        Ok(None) => {
            let content = "Looks like the replay for that score is not available";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get replay"));
        }
    };

    let (skin, settings) = match settings_res {
        Ok(tuple) => tuple,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    // Just a status update, no need to propagate an error
    status.set(RenderStatusInner::CommissioningRender);
    let _ = command.update(&ctx, status.as_message()).await;

    let render_fut = ctx
        .ordr()
        .client()
        .render_with_replay_file(&replay, RENDERER_NAME, &skin)
        .options(&settings);

    let render = match render_fut.await {
        Ok(render) => render,
        Err(err) => {
            let _ = command.error(&ctx, ORDR_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to commission render"));
        }
    };

    let ongoing = OngoingRender::new(ctx, render.render_id, command, status, Some(score_id)).await;

    tokio::spawn(ongoing.await_render_url());

    Ok(())
}

pub struct RenderStatus {
    start: RenderStatusInner,
    curr: RenderStatusInner,
}

impl RenderStatus {
    pub fn new_requesting_score() -> Self {
        Self {
            start: RenderStatusInner::RequestingScore,
            curr: RenderStatusInner::RequestingScore,
        }
    }

    pub fn new_preparing_replay() -> Self {
        Self {
            start: RenderStatusInner::PreparingReplay,
            curr: RenderStatusInner::PreparingReplay,
        }
    }

    pub fn new_commisioning_replay() -> Self {
        Self {
            start: RenderStatusInner::CommissioningRender,
            curr: RenderStatusInner::CommissioningRender,
        }
    }

    pub fn set(&mut self, status: RenderStatusInner) {
        self.curr = status;
    }

    pub fn as_message(&self) -> MessageBuilder<'static> {
        fn preparation_done_emote(
            start: &RenderStatusInner,
            check: &RenderStatusInner,
        ) -> ProgressEmote {
            match (start, check) {
                (RenderStatusInner::PreparingReplay, RenderStatusInner::RequestingScore) => {
                    ProgressEmote::Skipped
                }
                (
                    RenderStatusInner::CommissioningRender,
                    RenderStatusInner::RequestingScore | RenderStatusInner::PreparingReplay,
                ) => ProgressEmote::Skipped,
                _ => ProgressEmote::Done,
            }
        }

        fn description(
            requesting: ProgressEmote,
            preparing: ProgressEmote,
            commissioning: ProgressEmote,
            rendering: ProgressEmote,
            rendering_text: &str,
        ) -> String {
            format!(
                "- Requesting score {requesting}\n\
                - Preparing replay {preparing}\n\
                - Commissioning render {commissioning}\n\
                - {rendering_text} {rendering}"
            )
        }

        let content = match self.curr {
            RenderStatusInner::RequestingScore => description(
                ProgressEmote::Running,
                ProgressEmote::Waiting,
                ProgressEmote::Waiting,
                ProgressEmote::Waiting,
                "Rendering",
            ),
            RenderStatusInner::PreparingReplay => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                ProgressEmote::Running,
                ProgressEmote::Waiting,
                ProgressEmote::Waiting,
                "Rendering",
            ),
            RenderStatusInner::CommissioningRender => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                preparation_done_emote(&self.start, &RenderStatusInner::PreparingReplay),
                ProgressEmote::Running,
                ProgressEmote::Waiting,
                "Rendering",
            ),
            RenderStatusInner::Rendering(ref rendering) => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                preparation_done_emote(&self.start, &RenderStatusInner::PreparingReplay),
                ProgressEmote::Done,
                ProgressEmote::Running,
                rendering,
            ),
        };

        let embed = EmbedBuilder::new()
            .description(content)
            .title("Render status")
            .url("https://ordr.issou.best/renders");

        MessageBuilder::new().embed(embed)
    }
}

pub enum RenderStatusInner {
    RequestingScore,
    PreparingReplay,
    CommissioningRender,
    Rendering(Box<str>),
}

enum ProgressEmote {
    Done,
    Running,
    Skipped,
    Waiting,
}

impl Display for ProgressEmote {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Done => f.write_str("✅"),
            Self::Running => f.write_str("🏃‍♂️"),
            Self::Skipped => f.write_str("⏭️"),
            Self::Waiting => f.write_str("⌛"),
        }
    }
}

pub struct OngoingRender {
    ctx: Arc<Context>,
    render_id: u32,
    orig: OwnedCommandOrigin,
    status: RenderStatus,
    receivers: OrdrReceivers,
    score_id: Option<u64>,
}

impl OngoingRender {
    pub async fn new(
        ctx: Arc<Context>,
        render_id: u32,
        orig: impl Into<OwnedCommandOrigin>,
        status: RenderStatus,
        score_id: Option<u64>,
    ) -> Self {
        Self {
            orig: orig.into(),
            render_id,
            receivers: ctx.ordr().subscribe_render_id(render_id).await,
            status,
            ctx,
            score_id,
        }
    }

    pub async fn await_render_url(mut self) {
        const MINUTE: Duration = Duration::from_secs(60);
        const INTERVAL: Duration = Duration::from_secs(5);

        let mut last_update = Instant::now();

        loop {
            tokio::select! {
                progress = self.receivers.progress.recv() => {
                    let now = Instant::now();

                    if last_update + INTERVAL > now {
                        continue;
                    }

                    last_update = now;

                    let Some(progress) = progress else {
                        return warn!("progress channel was closed");
                    };


                    self.status.set(RenderStatusInner::Rendering(progress.progress));
                    let builder = self.status.as_message();

                    if let Err(err) = self.orig.update(&self.ctx, builder).await {
                        warn!(?err, "Failed to update message");
                    }
                },
                done = self.receivers.done.recv() => {
                    let Some(done) = done else {
                        return warn!("done channel was closed");
                    };

                    let builder = MessageBuilder::new().content(done.video_url.as_ref()).embed(None);

                    if let Err(err) = self.orig.update(&self.ctx, builder).await {
                        warn!(?err, "Failed to update message");
                    }

                    self.ctx.ordr().unsubscribe_render_id(done.render_id).await;

                    if let Some(score_id) = self.score_id {
                        let replay_manager = self.ctx.replay();
                        let store_fut = replay_manager.store_video_url(score_id, done.video_url.as_ref());

                        if let Err(err) = store_fut.await {
                            warn!(?err, "Failed to store video url");
                        }
                    }

                    return;
                },
                failed = self.receivers.failed.recv() => {
                    let Some(failed) = failed else {
                        return warn!("failed channel was closed");
                    };

                    warn!(?failed, "Received error from o!rdr");

                    let embed = EmbedBuilder::new().description(failed.error_message).color_red();
                    let builder = MessageBuilder::new().embed(embed);

                    if let Err(err) = self.orig.update(&self.ctx, builder).await {
                        warn!(?err, "Failed to update message");
                    }

                    self.ctx.ordr().unsubscribe_render_id(failed.render_id).await;

                    return;
                },
                _ = tokio::time::sleep(MINUTE) => {
                    let content = "Timeout while waiting for o!rdr updates, \
                        there was probably a network issue.";

                    if let Err(err) = self.orig.error(&self.ctx, content).await {
                        warn!(?err, "Failed to update message");
                    }

                    self.ctx.ordr().unsubscribe_render_id(self.render_id).await;

                    return;
                },
            }
        }
    }
}

async fn render_settings_modify(ctx: Arc<Context>, command: &mut InteractionCommand) -> Result<()> {
    command
        .defer(&ctx, false)
        .await
        .wrap_err("Failed to defer command")?;

    let owner = command.user_id()?;

    let (skin, settings) = match ctx.replay().get_settings(owner).await {
        Ok(tuple) => tuple,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let active = RenderSettingsActive::new(skin, settings, None, owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(ctx, command)
        .await
}

async fn render_settings_import(ctx: Arc<Context>, command: &mut InteractionCommand) -> Result<()> {
    ActiveMessages::builder(SettingsImport::new(command.user_id()?))
        .begin(ctx, command)
        .await
}

async fn render_settings_copy(
    ctx: Arc<Context>,
    command: &mut InteractionCommand,
    args: RenderSettingsCopy,
) -> Result<()> {
    command
        .defer(&ctx, false)
        .await
        .wrap_err("Failed to defer command")?;

    let owner = command.user_id()?;
    let replay_manager = ctx.replay();

    let (skin, settings) = match replay_manager.get_settings(args.user).await {
        Ok(tuple) => tuple,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if let Err(err) = replay_manager.set_settings(owner, &skin, &settings).await {
        let _ = command.error(&ctx, GENERAL_ISSUE).await;

        return Err(err);
    }

    let content = "Settings copied successfully";
    let active = RenderSettingsActive::new(skin, settings, Some(content), owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(ctx, command)
        .await
}

async fn render_settings_default(
    ctx: Arc<Context>,
    command: &mut InteractionCommand,
) -> Result<()> {
    command
        .defer(&ctx, false)
        .await
        .wrap_err("Failed to defer command")?;

    let owner = command.user_id()?;
    let replay_manager = ctx.replay();

    let (skin, settings) = replay_manager.get_default_settings();

    if let Err(err) = replay_manager.set_settings(owner, &skin, &settings).await {
        let _ = command.error(&ctx, GENERAL_ISSUE).await;

        return Err(err);
    }

    let content = "Settings reset to default successfully";
    let active = RenderSettingsActive::new(skin, settings, Some(content), owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(ctx, command)
        .await
}
