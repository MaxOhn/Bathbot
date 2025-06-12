use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::{Duration, Instant},
};

use bathbot_macros::SlashCommand;
use bathbot_util::{
    Authored, BucketName, EmbedBuilder, MessageBuilder,
    constants::{GENERAL_ISSUE, ORDR_ISSUE, OSU_API_ISSUE},
    matcher,
};
use eyre::{Report, Result, WrapErr};
use rosu_render::{
    client::error::{ApiError as OrdrApiError, ClientError as OrdrError},
    model::RenderDone,
};
use rosu_v2::error::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{Attachment, Message},
    guild::Permissions,
    id::{
        Id,
        marker::{ChannelMarker, MessageMarker, UserMarker},
    },
};

use crate::{
    active::{
        ActiveMessages,
        impls::{CachedRender, RenderSettingsActive, SettingsImport},
    },
    core::{Context, commands::OwnedCommandOrigin},
    manager::{ReplayError, ReplaySettings},
    tracking::OrdrReceivers,
    util::{InteractionCommandExt, MessageExt, interaction::InteractionCommand},
};

pub const RENDERER_NAME: &str = "Bathbot";

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "render",
    desc = "Render an osu!standard play via o!rdr",
    help = "Render a play via [o!rdr](https://ordr.issou.best/).\n\
    Since [danser](https://github.com/Wieku/danser-go) is being used, \
    only osu!standard is supported."
)]
#[flags(SKIP_DEFER)]
#[allow(clippy::large_enum_variant)]
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
    #[command(desc = "Specify the score through its id or url")]
    score_id: String,
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

pub async fn slash_render(mut command: InteractionCommand) -> Result<()> {
    if !Context::ordr_available() {
        command
            .error_callback("Rendering is currently unavailable")
            .await?;

        return Ok(());
    };

    match Render::from_interaction(command.input_data())? {
        Render::Replay(args) => render_replay(command, args).await,
        Render::Score(args) => render_score(command, args).await,
        Render::Settings(RenderSettings::Modify(_)) => render_settings_modify(&mut command).await,
        Render::Settings(RenderSettings::Import(_)) => render_settings_import(&mut command).await,
        Render::Settings(RenderSettings::Copy(args)) => {
            render_settings_copy(&mut command, args).await
        }
        Render::Settings(RenderSettings::Default(_)) => render_settings_default(&mut command).await,
    }
}

async fn render_replay(command: InteractionCommand, replay: RenderReplay) -> Result<()> {
    let owner = command.user_id()?;

    if let Some(cooldown) = Context::check_ratelimit(owner, BucketName::Render) {
        trace!("Ratelimiting user {owner} on bucket `Render` for {cooldown} seconds");

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        command.error_callback(content).await?;

        return Ok(());
    }

    let RenderReplay { replay } = replay;

    if !replay.filename.ends_with(".osr") {
        let content = "The attached replay must be a .osr file";
        command.error_callback(content).await?;

        return Ok(());
    }

    let status = RenderStatus::new_commissioning_replay();
    command.callback(status.as_message(), false).await?;

    let settings = match Context::replay().get_settings(owner).await {
        Ok(settings) => settings,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let allow_custom_skins = match command.guild_id {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| config.allow_custom_skins.unwrap_or(true))
                .await
        }
        None => true,
    };

    let skin = settings.skin(allow_custom_skins);

    debug!(
        replay_url = replay.url,
        discord = owner.get(),
        "Commissioning render"
    );

    let render_fut = Context::ordr()
        .client()
        .render_with_replay_url(&replay.url, RENDERER_NAME, &skin.skin)
        .options(settings.options());

    let render = match render_fut.await {
        Ok(render) => render,
        Err(err) => {
            return match err {
                OrdrError::Response {
                    error:
                        OrdrApiError {
                            code: Some(code), ..
                        },
                    ..
                } => {
                    let content =
                        format!("Error code {int} from o!rdr: {code}", int = code.to_u8());
                    command.error(content).await?;

                    Ok(())
                }
                _ => {
                    let _ = command.error(ORDR_ISSUE).await;

                    Err(Report::new(err).wrap_err("Failed to commission render"))
                }
            };
        }
    };

    let response = match Context::interaction().response(&command.token).await {
        Ok(response) => match response.model().await {
            Ok(msg) => Some(msg),
            Err(err) => {
                warn!(err = ?Report::new(err), "Failed to deserialize response");

                None
            }
        },
        Err(err) => {
            warn!(err = ?Report::new(err), "Failed to fetch response");

            None
        }
    };

    let ongoing = OngoingRender::new(
        render.render_id,
        &command,
        ProgressResponse::new(response, command.permissions, false),
        status,
        None,
        owner,
    )
    .await;

    tokio::spawn(ongoing.await_render_url());

    Ok(())
}

async fn render_score(mut command: InteractionCommand, score: RenderScore) -> Result<()> {
    command.defer(false).await?;

    let owner = command.user_id()?;
    let RenderScore { score_id } = score;

    // Parse score id
    let score_id = match score_id.parse() {
        Ok(score_id) => score_id,
        Err(_) => match matcher::get_osu_score_id(&score_id) {
            Some((score_id, _)) => score_id,
            None => {
                let content = "Must give either a score id or url";
                command.error(content).await?;

                return Ok(());
            }
        },
    };

    // Check if the score id has already been rendered
    match Context::replay().get_video_url(score_id).await {
        Ok(Some(video_url)) => {
            let cached = CachedRender::new(score_id, video_url, false, owner);

            return ActiveMessages::builder(cached)
                .start_by_update(true)
                .begin(&mut command)
                .await;
        }
        Ok(None) => {}
        Err(err) => warn!(?err),
    }

    if let Some(cooldown) = Context::check_ratelimit(owner, BucketName::Render) {
        trace!("Ratelimiting user {owner} on bucket `Render` for {cooldown} seconds");

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        command.error(content).await?;

        return Ok(());
    }

    let mut status = RenderStatus::new_requesting_score();
    command.update(status.as_message()).await?;

    // Just a status update, no need to propagate an error
    status.set(RenderStatusInner::PreparingReplay);
    let _ = command.update(status.as_message()).await;

    let replay_manager = Context::replay();
    let replay_fut = replay_manager.get_replay(score_id);
    let settings_fut = replay_manager.get_settings(owner);

    let (replay_res, settings_res) = tokio::join!(replay_fut, settings_fut);

    let replay = match replay_res {
        Ok(Some(replay)) => replay,
        Ok(None) => {
            let content = "Looks like the replay for that score is not available";
            command.error(content).await?;

            return Ok(());
        }
        Err(ReplayError::Osu(OsuError::NotFound)) => {
            let content = "Found no score with that id";
            command.error(content).await?;

            return Ok(());
        }
        Err(ReplayError::Osu(err)) => {
            let _ = command.error(OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get replay"));
        }
        Err(ReplayError::AlreadyRequestedCheck(err)) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err(ReplayError::ALREADY_REQUESTED_TEXT));
        }
    };

    let settings = match settings_res {
        Ok(settings) => settings,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    status.set(RenderStatusInner::CommissioningRender);

    let response = match command.update(status.as_message()).await {
        Ok(response) => match response.model().await {
            Ok(msg) => Some(msg),
            Err(err) => {
                warn!(err = ?Report::new(err), "Failed to deserialize response");

                None
            }
        },
        Err(err) => {
            warn!(err = ?Report::new(err), "Failed to respond");

            None
        }
    };

    let allow_custom_skins = match command.guild_id {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| config.allow_custom_skins.unwrap_or(true))
                .await
        }
        None => true,
    };

    let skin = settings.skin(allow_custom_skins);

    debug!(score_id, discord = owner.get(), "Commissioning render");

    let render_fut = Context::ordr()
        .client()
        .render_with_replay_file(&replay, RENDERER_NAME, &skin.skin)
        .options(settings.options());

    let render = match render_fut.await {
        Ok(render) => render,
        Err(OrdrError::Response {
            error: OrdrApiError {
                code: Some(code), ..
            },
            ..
        }) => {
            let content = format!("Error code {int} from o!rdr: {code}", int = code.to_u8());
            command.error(content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(ORDR_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to commission render"));
        }
    };

    let ongoing_fut = OngoingRender::new(
        render.render_id,
        &command,
        ProgressResponse::new(response, command.permissions, false),
        status,
        Some(score_id),
        owner,
    );

    tokio::spawn(ongoing_fut.await.await_render_url());

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

    pub fn new_commissioning_replay() -> Self {
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
            rendering_text: Option<&str>,
        ) -> String {
            format!(
                "- Requesting score {requesting}\n\
                - Preparing replay {preparing}\n\
                - Commissioning render {commissioning}\n\
                - {} {rendering}",
                rendering_text.unwrap_or("Rendering")
            )
        }

        let content = match self.curr {
            RenderStatusInner::RequestingScore => description(
                ProgressEmote::Running,
                ProgressEmote::Waiting,
                ProgressEmote::Waiting,
                ProgressEmote::Waiting,
                None,
            ),
            RenderStatusInner::PreparingReplay => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                ProgressEmote::Running,
                ProgressEmote::Waiting,
                ProgressEmote::Waiting,
                None,
            ),
            RenderStatusInner::CommissioningRender => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                preparation_done_emote(&self.start, &RenderStatusInner::PreparingReplay),
                ProgressEmote::Running,
                ProgressEmote::Waiting,
                None,
            ),
            RenderStatusInner::Rendering(ref rendering) => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                preparation_done_emote(&self.start, &RenderStatusInner::PreparingReplay),
                ProgressEmote::Done,
                ProgressEmote::Running,
                Some(rendering),
            ),
            RenderStatusInner::Done => description(
                preparation_done_emote(&self.start, &RenderStatusInner::RequestingScore),
                preparation_done_emote(&self.start, &RenderStatusInner::PreparingReplay),
                ProgressEmote::Done,
                ProgressEmote::Done,
                None,
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
    Done,
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
    render_id: u32,
    // The original message that will be replied to
    orig: OwnedCommandOrigin,
    // The message that will be updated and deleted
    response: Option<ProgressResponse>,
    status: RenderStatus,
    receivers: OrdrReceivers,
    score_id: Option<u64>,
    msg_owner: Id<UserMarker>,
}

pub struct ProgressResponse {
    msg: Id<MessageMarker>,
    channel: Id<ChannelMarker>,
    permissions: Option<Permissions>,
    /// Whether the response should be deleted afterwards
    delete: bool,
}

impl ProgressResponse {
    pub fn new(
        msg: Option<Message>,
        permissions: Option<Permissions>,
        delete: bool,
    ) -> Option<Self> {
        msg.map(|msg| Self {
            msg: msg.id,
            channel: msg.channel_id,
            permissions,
            delete,
        })
    }

    fn get(&self) -> (Id<MessageMarker>, Id<ChannelMarker>) {
        (self.msg, self.channel)
    }
}

impl OngoingRender {
    pub async fn new(
        render_id: u32,
        orig: impl Into<OwnedCommandOrigin>,
        response: Option<ProgressResponse>,
        status: RenderStatus,
        score_id: Option<u64>,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            orig: orig.into(),
            response,
            render_id,
            receivers: Context::ordr().subscribe_render_id(render_id).await,
            status,
            score_id,
            msg_owner,
        }
    }

    pub async fn await_render_url(mut self) {
        const TIMEOUT_DURATION: Duration = Duration::from_secs(60 * 60 * 24);
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

                    if let Some(ref response) = self.response {
                        let perms = response.permissions;

                        if let Some(update_fut) = response.get().update(builder, perms) {
                            if let Err(err) = update_fut.await {
                                warn!(?err, "Failed to update message");
                            }
                        } else {
                            warn!("Lacking permissions to update message");
                        }
                    }
                },
                done = self.receivers.done.recv() => {
                    let Some(RenderDone { render_id, video_url }) = done else {
                        return warn!("done channel was closed");
                    };

                    if let Some(score_id) = self.score_id {
                        let replay_manager = Context::replay();
                        let store_fut = replay_manager.store_video_url(score_id, video_url.as_ref());

                        if let Err(err) = store_fut.await {
                            warn!(?err, score_id, video_url, "Failed to store video url");
                        } else {
                            debug!(score_id, video_url, "Stored render video url");
                        }
                    } else {
                        debug!("Missing score id, skip storing video url");
                    }

                    let video_url_with_user = format!("{video_url} <@{}>", self.msg_owner);
                    let builder = MessageBuilder::new().content(video_url_with_user).embed(None);

                    if let Err(err) = self.orig.reply(builder).await {
                        warn!(?err, "Failed to reply message");
                    } else if let Some(ref response) = self.response {

                        if response.delete {
                            if let Err(err) = response.get().delete().await {
                                warn!(?err, "Failed to delete response");
                            }
                        } else {
                            self.status.set(RenderStatusInner::Done);
                            let builder = self.status.as_message();
                            let perms = response.permissions;

                            if let Some(update_fut) = response.get().update(builder, perms) {
                                if let Err(err) = update_fut.await {
                                    warn!(?err, "Failed to update message");
                                }
                            } else {
                                warn!("Lacking permissions to update message");
                            }
                        }
                    }

                    Context::ordr().unsubscribe_render_id(render_id).await;

                    return;
                },
                failed = self.receivers.failed.recv() => {
                    let Some(failed) = failed else {
                        return warn!("failed channel was closed");
                    };

                    warn!(?failed, "Received error from o!rdr");

                    if let Err(err) = self.orig.reply_error(failed.error_message).await {
                        warn!(?err, "Failed to update message");
                    } else if let Some(ref response) = self.response {
                        if response.delete {
                            if let Err(err) = response.get().delete().await {
                                warn!(?err, "Failed to delete response");
                            }
                        } else {
                            let embed = EmbedBuilder::new()
                                .color_red()
                                .description("Render failed");
                            let builder = MessageBuilder::new().embed(embed);
                            let perms = response.permissions;

                            if let Some(update_fut) = response.get().update(builder, perms) {
                                if let Err(err) = update_fut.await {
                                    warn!(?err, "Failed to update message");
                                }
                            } else {
                                warn!("Lacking permissions to update message");
                            }
                        }
                    }

                    Context::ordr().unsubscribe_render_id(failed.render_id).await;

                    return;
                },
                _ = tokio::time::sleep(TIMEOUT_DURATION) => {
                    let content = "Timeout while waiting for o!rdr updates, \
                        there was probably a network issue.";

                    if let Err(err) = self.orig.reply_error(content).await {
                        warn!(?err, "Failed to update message");
                    } else if let Some(ref response) = self.response {
                        if response.delete {
                            if let Err(err) = response.get().delete().await {
                                warn!(?err, "Failed to delete response");
                            }
                        } else {
                            let embed = EmbedBuilder::new()
                                .color_red()
                                .description("Render failed");
                            let builder = MessageBuilder::new().embed(embed);
                            let perms = response.permissions;

                            if let Some(update_fut) = response.get().update(builder, perms) {
                                if let Err(err) = update_fut.await {
                                    warn!(?err, "Failed to update message");
                                }
                            } else {
                                warn!("Lacking permissions to update message");
                            }
                        }
                    }

                    Context::ordr().unsubscribe_render_id(self.render_id).await;

                    return;
                },
            }
        }
    }
}

async fn render_settings_modify(command: &mut InteractionCommand) -> Result<()> {
    command
        .defer(false)
        .await
        .wrap_err("Failed to defer command")?;

    let owner = command.user_id()?;

    let settings = match Context::replay().get_settings(owner).await {
        Ok(settings) => settings,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let active = RenderSettingsActive::new(settings, None, owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(command)
        .await
}

async fn render_settings_import(command: &mut InteractionCommand) -> Result<()> {
    ActiveMessages::builder(SettingsImport::new(command.user_id()?))
        .begin(command)
        .await
}

async fn render_settings_copy(
    command: &mut InteractionCommand,
    args: RenderSettingsCopy,
) -> Result<()> {
    command
        .defer(false)
        .await
        .wrap_err("Failed to defer command")?;

    let owner = command.user_id()?;
    let replay_manager = Context::replay();

    let settings = match replay_manager.get_settings(args.user).await {
        Ok(settings) => settings,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if let Err(err) = replay_manager.set_settings(owner, &settings).await {
        let _ = command.error(GENERAL_ISSUE).await;

        return Err(err);
    }

    let content = "Settings copied successfully";
    let active = RenderSettingsActive::new(settings, Some(content), owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(command)
        .await
}

async fn render_settings_default(command: &mut InteractionCommand) -> Result<()> {
    command
        .defer(false)
        .await
        .wrap_err("Failed to defer command")?;

    let owner = command.user_id()?;
    let replay_manager = Context::replay();
    let settings = ReplaySettings::default();

    if let Err(err) = replay_manager.set_settings(owner, &settings).await {
        let _ = command.error(GENERAL_ISSUE).await;

        return Err(err);
    }

    let content = "Settings reset to default successfully";
    let active = RenderSettingsActive::new(settings, Some(content), owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(command)
        .await
}
