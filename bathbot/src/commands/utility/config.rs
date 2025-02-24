use ::time::UtcOffset;
use bathbot_macros::{SlashCommand, command};
use bathbot_model::command_fields::{ShowHideOption, TimezoneOption};
use bathbot_psql::model::configs::{
    ListSize, OsuUserId, OsuUsername, Retries, ScoreData, UserConfig,
};
#[cfg(feature = "server")]
use bathbot_server::AuthenticationStandbyError;
use bathbot_util::constants::GENERAL_ISSUE;
#[cfg(feature = "server")]
use bathbot_util::{EmbedBuilder, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{Id, marker::UserMarker};

use super::{SkinValidation, ValidationStatus};
use crate::{
    Context,
    embeds::{ConfigEmbed, EmbedData},
    util::{Authored, InteractionCommandExt, interaction::InteractionCommand},
};
#[cfg(feature = "server")]
use crate::{core::BotConfig, util::Emote};

#[cfg(feature = "server")]
#[derive(CommandModel, CreateCommand, Default, SlashCommand)]
#[command(
    name = "config",
    desc = "Adjust your default configuration for commands"
)]
#[flags(EPHEMERAL)]
pub struct Config {
    #[command(
        desc = "Specify whether you want to link to an osu! profile",
        help = "Most osu! commands require a specified username to work.\n\
        Since using a command is most commonly intended for your own profile, you can link \
        your discord with an osu! profile so that when no username is specified in commands, \
        it will choose the linked username.\n\
        If the value is set to `Link`, it will prompt you to authorize your account.\n\
        If `Unlink` is selected, you will be unlinked from the osu! profile."
    )]
    pub osu: Option<ConfigLink>,
    #[command(
        desc = "Specify whether you want to link to a twitch profile",
        help = "With this option you can link to a twitch channel.\n\
        When you have both your osu! and twitch linked, are currently streaming, and anyone uses \
        the `recent score` command on your osu! username, it will try to retrieve the last VOD from your \
        twitch channel and link to a timestamp for the score.\n\
        If the value is set to `Link`, it will prompt you to authorize your account.\n\
        If `Unlink` is selected, you will be unlinked from the twitch channel."
    )]
    twitch: Option<ConfigLink>,
    #[command(
        desc = "Specify a gamemode (NOTE: Only use for non-std modes if you NEVER use std commands)",
        help = "Always having to specify the `mode` option for any non-std \
        command can be pretty tedious.\nTo get around that, you can configure a mode here so \
        that when the `mode` option is not specified in commands, it will choose your config mode."
    )]
    mode: Option<ConfigGameMode>,
    #[command(
        desc = "Adjust the amount of scores shown per page in top, rb, pinned, ...",
        help = "Adjust the amount of scores shown per page in `/top`, `/rb`, `/pinned`, and `/mapper`.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1."
    )]
    list_embeds: Option<ListSize>,
    #[command(desc = "Should the amount of retries be shown for the recent command?")]
    retries: Option<Retries>,
    #[command(desc = "Specify a timezone which will be used for commands like `/graph`")]
    timezone: Option<TimezoneOption>,
    #[command(
        desc = "Specify a download link for your skin",
        help = "Specify a download link for your skin.\n\
        Must be a URL to a direct-download of an .osk file or one of these approved sites:\n\
        - `https://drive.google.com`\n\
        - `https://www.dropbox.com`\n\
        - `https://mega.nz`\n\
        - `https://www.mediafire.com`\n\
        - `https://skins.osuck.net`\n\
        If you want to suggest another site let Badewanne3 know."
    )]
    skin_url: Option<String>,
    #[command(
        desc = "Should the recent command include a render button?",
        help = "Should the `recent` command include a render button?\n\
        The button would be a shortcut for the `/render` command.\n\
        In servers, this requires that the render button is not disabled in `/serverconfigs`."
    )]
    render_button: Option<ShowHideOption>,
    #[command(
        desc = "Whether scores should be requested as lazer or stable scores",
        help = "Whether scores should be requested as lazer or stable scores.\n\
        They have a different score and grade calculation and only lazer adds the new mods."
    )]
    score_data: Option<ScoreData>,
}

// FIXME: Some attribute command does not register the #[cfg(feature = "")]
// tag on fields so we need an entirely new struct for now
#[cfg(not(feature = "server"))]
#[derive(CommandModel, CreateCommand, Default, SlashCommand)]
#[command(
    name = "config",
    desc = "Adjust your default configuration for commands"
)]
#[flags(EPHEMERAL)]
pub struct Config {
    #[command(
        desc = "Specify a gamemode (NOTE: Only use for non-std modes if you NEVER use std commands)",
        help = "Always having to specify the `mode` option for any non-std \
        command can be pretty tedious.\nTo get around that, you can configure a mode here so \
        that when the `mode` option is not specified in commands, it will choose your config mode."
    )]
    mode: Option<ConfigGameMode>,
    #[command(
        desc = "Adjust the amount of scores shown per page in top, rb, pinned, ...",
        help = "Adjust the amount of scores shown per page in top, rb, pinned, and mapper.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1."
    )]
    list_embeds: Option<ListSize>,
    #[command(desc = "Specify if and how retries should be shown for the recent command")]
    retries: Option<Retries>,
    #[command(desc = "Specify a timezone which will be used for commands like `/graph`")]
    timezone: Option<TimezoneOption>,
    #[command(
        desc = "Specify a download link for your skin",
        help = "Specify a download link for your skin.\n\
        Must be a URL to a direct-download of an .osk file or one of these approved sites:\n\
        - `https://drive.google.com`\n\
        - `https://www.dropbox.com`\n\
        - `https://mega.nz`\n\
        - `https://www.mediafire.com`\n\
        - `https://skins.osuck.net`\n\
        If you want to suggest another site let Badewanne3 know."
    )]
    skin_url: Option<String>,
    #[command(
        desc = "Should the recent command include a render button?",
        help = "Should the `recent` command include a render button?\n\
        The button would be a shortcut for the `/render` command.\n\
        In servers, this requires that the render button is not disabled in `/serverconfigs`."
    )]
    render_button: Option<ShowHideOption>,
    #[command(
        desc = "Whether scores should be requested as lazer or stable scores",
        help = "Whether scores should be requested as lazer or stable scores.\n\
        They have a different score and grade calculation and only lazer adds the new mods."
    )]
    score_data: Option<ScoreData>,
}

#[derive(CommandOption, CreateOption)]
pub enum ConfigLink {
    #[option(name = "Link", value = "link")]
    Link,
    #[option(name = "Unlink", value = "unlink")]
    Unlink,
}

#[derive(CommandOption, CreateOption)]
pub enum ConfigGameMode {
    #[option(name = "None", value = "none")]
    None,
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
    #[option(name = "mania", value = "mania")]
    Mania,
}

impl From<ConfigGameMode> for Option<GameMode> {
    fn from(mode: ConfigGameMode) -> Self {
        match mode {
            ConfigGameMode::None => None,
            ConfigGameMode::Osu => Some(GameMode::Osu),
            ConfigGameMode::Taiko => Some(GameMode::Taiko),
            ConfigGameMode::Catch => Some(GameMode::Catch),
            ConfigGameMode::Mania => Some(GameMode::Mania),
        }
    }
}

async fn slash_config(mut command: InteractionCommand) -> Result<()> {
    let args = Config::from_interaction(command.input_data())?;

    config(command, args).await
}

pub async fn config(command: InteractionCommand, config: Config) -> Result<()> {
    let Config {
        #[cfg(feature = "server")]
        osu,
        #[cfg(feature = "server")]
        twitch,
        mode,
        list_embeds,
        retries,
        timezone,
        mut skin_url,
        render_button,
        score_data,
    } = config;

    if let Some(ref skin_url) = skin_url {
        match SkinValidation::check(&command, skin_url).await? {
            ValidationStatus::Continue => {}
            ValidationStatus::Handled => return Ok(()),
        }
    }

    let author = command.user()?;

    let mut config = match Context::user_config().with_osu_id(author.id).await {
        Ok(config) => config,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    match mode {
        None => {}
        Some(ConfigGameMode::None) => config.mode = None,
        Some(ConfigGameMode::Osu) => config.mode = Some(GameMode::Osu),
        Some(ConfigGameMode::Taiko) => config.mode = Some(GameMode::Taiko),
        Some(ConfigGameMode::Catch) => config.mode = Some(GameMode::Catch),
        Some(ConfigGameMode::Mania) => config.mode = Some(GameMode::Mania),
    }

    if let Some(list_embeds) = list_embeds {
        config.list_size = Some(list_embeds);
    }

    if let Some(retries) = retries {
        config.retries = Some(retries);
    }

    if let Some(tz) = timezone.map(UtcOffset::from) {
        config.timezone = Some(tz);
    }

    if let Some(render_button) = render_button {
        config.render_button = Some(matches!(render_button, ShowHideOption::Show));
    }

    if let Some(score_data) = score_data {
        config.score_data = Some(score_data);
    }

    #[cfg(feature = "server")]
    if let Some(ConfigLink::Unlink) = osu {
        config.osu.take();
    }

    #[cfg(feature = "server")]
    if let Some(ConfigLink::Unlink) = twitch {
        config.twitch_id.take();
    }

    #[cfg(feature = "server")]
    let res = {
        match (osu, twitch) {
            (Some(ConfigLink::Link), Some(ConfigLink::Link)) => {
                handle_both_links(&command, &mut config).await
            }
            (Some(ConfigLink::Link), _) => handle_osu_link(&command, &mut config).await,
            (_, Some(ConfigLink::Link)) => handle_twitch_link(&command, &mut config).await,
            (..) => handle_no_links(&command, &mut config).await,
        }
    };

    #[cfg(not(feature = "server"))]
    let res = handle_no_links(&command, &mut config).await;

    match res {
        HandleResult::TwitchName(twitch_name) => {
            let config = if let Some(ref skin_url) = skin_url {
                let update_fut = Context::user_config().update_skin(author.id, Some(skin_url));

                if let Err(err) = update_fut.await {
                    command.error(GENERAL_ISSUE).await?;

                    return Err(err);
                }

                convert_config(config, author.id).await
            } else {
                let config_fut = convert_config(config, author.id);
                let skin_fut = Context::user_config().skin(author.id);
                let (config, skin_res) = tokio::join!(config_fut, skin_fut);

                match skin_res {
                    Ok(skin_) => skin_url = skin_,
                    Err(err) => error!("{err:?}"),
                }

                config
            };

            let embed_data = ConfigEmbed::new(author, config, twitch_name, skin_url);
            let builder = embed_data.build().into();
            command.update(builder).await?;

            Ok(())
        }
        HandleResult::Done => Ok(()),
        HandleResult::Err(err) => Err(err.wrap_err("failed to handle config update")),
    }
}

#[cfg(feature = "server")]
const MSG_BADE: &str = "Contact Badewanne3 if you encounter issues with the website";

#[cfg(feature = "server")]
fn osu_content(state: u8) -> String {
    let config = BotConfig::get();

    format!(
        "{emote} [Click here](https://osu.ppy.sh/oauth/authorize?client_id={client_id}&\
        response_type=code&scope=identify&redirect_uri={url}/auth/osu&state={state}) \
        to authenticate your osu! profile",
        emote = Emote::Osu,
        client_id = config.tokens.osu_client_id,
        url = config.server.public_url,
    )
}

#[cfg(feature = "server")]
fn twitch_content(state: u8) -> String {
    let config = BotConfig::get();

    format!(
        "{emote} [Click here](https://id.twitch.tv/oauth2/authorize?client_id={client_id}\
        &response_type=code&scope=user:read:email&redirect_uri={url}/auth/twitch\
        &state={state}) to authenticate your twitch channel",
        emote = Emote::Twitch,
        client_id = config.tokens.twitch_client_id,
        url = config.server.public_url,
    )
}

#[cfg(feature = "server")]
async fn handle_both_links(
    command: &InteractionCommand,
    config: &mut UserConfig<OsuUserId>,
) -> HandleResult {
    let auth_standby = Context::auth_standby();
    let osu_fut = auth_standby.wait_for_osu();
    let twitch_fut = auth_standby.wait_for_twitch();

    let content = format!(
        "{}\n{}",
        osu_content(osu_fut.state),
        twitch_content(twitch_fut.state)
    );

    let embed = EmbedBuilder::new().description(content).footer(MSG_BADE);
    let builder = MessageBuilder::new().embed(embed);
    let fut = async { tokio::try_join!(osu_fut, twitch_fut) };

    let twitch_name = match handle_ephemeral(command, builder, fut).await {
        Some(Ok((osu, twitch))) => {
            config.osu = Some(osu.user_id);
            config.twitch_id = Some(twitch.user_id);

            tokio::spawn(async move {
                Context::osu_user().store(&osu, osu.mode).await;
            });

            Some(twitch.display_name)
        }
        Some(Err(err)) => return HandleResult::Err(err),
        None => return HandleResult::Done,
    };

    let author = match command.user() {
        Ok(author) => author,
        Err(err) => return HandleResult::Err(err),
    };

    if let Err(err) = Context::user_config().store(author.id, config).await {
        let _ = command.error(GENERAL_ISSUE).await;

        return HandleResult::Err(err);
    }

    HandleResult::TwitchName(twitch_name)
}

#[cfg(feature = "server")]
async fn handle_twitch_link(
    command: &InteractionCommand,
    config: &mut UserConfig<OsuUserId>,
) -> HandleResult {
    let fut = Context::auth_standby().wait_for_twitch();

    let embed = EmbedBuilder::new()
        .description(twitch_content(fut.state))
        .footer(MSG_BADE);

    let builder = MessageBuilder::new().embed(embed);

    let twitch_name = match handle_ephemeral(command, builder, fut).await {
        Some(Ok(user)) => {
            config.twitch_id = Some(user.user_id);

            Some(user.display_name)
        }
        Some(Err(err)) => return HandleResult::Err(err),
        None => return HandleResult::Done,
    };

    let author = match command.user() {
        Ok(author) => author,
        Err(err) => return HandleResult::Err(err),
    };

    if let Err(err) = Context::user_config().store(author.id, config).await {
        let _ = command.error(GENERAL_ISSUE).await;

        return HandleResult::Err(err);
    }

    HandleResult::TwitchName(twitch_name)
}

#[cfg(feature = "server")]
async fn handle_osu_link(
    command: &InteractionCommand,
    config: &mut UserConfig<OsuUserId>,
) -> HandleResult {
    let fut = Context::auth_standby().wait_for_osu();

    let embed = EmbedBuilder::new()
        .description(osu_content(fut.state))
        .footer(MSG_BADE);

    let builder = MessageBuilder::new().embed(embed);

    config.osu = match handle_ephemeral(command, builder, fut).await {
        Some(Ok(user)) => {
            let user_id = user.user_id;

            tokio::spawn(async move {
                Context::osu_user().store(&user, user.mode).await;
            });

            Some(user_id)
        }
        Some(Err(err)) => return HandleResult::Err(err),
        None => return HandleResult::Done,
    };

    let author = match command.user() {
        Ok(author) => author,
        Err(err) => return HandleResult::Err(err),
    };

    let mut twitch_name = None;

    if let Some(user_id) = config.twitch_id {
        match Context::client().get_twitch_user_by_id(user_id).await {
            Ok(Some(user)) => twitch_name = Some(user.display_name),
            Ok(None) => {
                debug!("No twitch user found for given id, remove from config");
                config.twitch_id.take();
            }
            Err(err) => {
                let _ = command
                    .error(bathbot_util::constants::TWITCH_API_ISSUE)
                    .await;

                return HandleResult::Err(err.wrap_err("failed to get twitch user by id"));
            }
        }
    }

    if let Err(err) = Context::user_config().store(author.id, config).await {
        let _ = command.error(GENERAL_ISSUE).await;

        return HandleResult::Err(err);
    }

    HandleResult::TwitchName(twitch_name)
}

#[cfg(feature = "server")]
async fn handle_ephemeral<T>(
    command: &InteractionCommand,
    builder: MessageBuilder<'_>,
    fut: impl std::future::Future<Output = Result<T, AuthenticationStandbyError>>,
) -> Option<Result<T>> {
    if let Err(err) = command.update(builder).await {
        return Some(Err(eyre::Report::new(err)));
    }

    let content = match fut.await {
        Ok(res) => return Some(Ok(res)),
        Err(AuthenticationStandbyError::Timeout) => "You did not authenticate in time",
        Err(AuthenticationStandbyError::Canceled) => GENERAL_ISSUE,
    };

    if let Err(err) = command.error(content).await {
        return Some(Err(err.into()));
    }

    None
}

async fn handle_no_links(
    command: &InteractionCommand,
    config: &mut UserConfig<OsuUserId>,
) -> HandleResult {
    let author = match command.user() {
        Ok(author) => author,
        Err(err) => return HandleResult::Err(err),
    };

    let mut twitch_name = None;

    if let Some(_user_id) = config.twitch_id {
        #[cfg(feature = "twitch")]
        match Context::client().get_twitch_user_by_id(_user_id).await {
            Ok(Some(user)) => twitch_name = Some(user.display_name),
            Ok(None) => {
                debug!("No twitch user found for given id, remove from config");
                config.twitch_id.take();
            }
            Err(err) => {
                let _ = command
                    .error(bathbot_util::constants::TWITCH_API_ISSUE)
                    .await;

                return HandleResult::Err(err.wrap_err("failed to get twitch user by id"));
            }
        }

        #[cfg(not(feature = "twitch"))]
        {
            twitch_name = Some(Box::from("?"));
        }
    }

    if let Err(err) = Context::user_config().store(author.id, config).await {
        let _ = command.error(GENERAL_ISSUE).await;

        return HandleResult::Err(err);
    }

    HandleResult::TwitchName(twitch_name)
}

async fn convert_config(
    config: UserConfig<OsuUserId>,
    user_id: Id<UserMarker>,
) -> UserConfig<OsuUsername> {
    let username = match Context::user_config().osu_name(user_id).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            warn!("Missing name for user config");

            "<failed to get name>".into()
        }
        Err(err) => {
            warn!("{err:?}");

            "<failed to get name>".into()
        }
    };

    let UserConfig {
        list_size,
        score_embed,
        mode,
        osu: _,
        retries,
        twitch_id,
        timezone,
        render_button,
        score_data,
    } = config;

    UserConfig {
        list_size,
        score_embed,
        mode,
        osu: Some(username),
        retries,
        twitch_id,
        timezone,
        render_button,
        score_data,
    }
}

enum HandleResult {
    TwitchName(Option<Box<str>>),
    #[allow(unused)]
    Done,
    Err(Report),
}
