use std::{future::Future, sync::Arc};

use command_macros::{command, SlashCommand};
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    commands::{osu::ProfileSize, ShowHideOption},
    core::CONFIG,
    database::{EmbedsSize, MinimizedPp, OsuData, UserConfig},
    embeds::{ConfigEmbed, EmbedData},
    server::AuthenticationStandbyError,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, TWITCH_API_ISSUE},
        ApplicationCommandExt, Authored, Emote,
    },
    BotResult, Context,
};

#[derive(CommandModel, CreateCommand, Default, SlashCommand)]
#[command(name = "config")]
#[flags(EPHEMERAL)]
/// Adjust your default configuration for commands
pub struct Config {
    #[command(help = "Most osu! commands require a specified username to work.\n\
    Since using a command is most commonly intended for your own profile, you can link \
    your discord with an osu! profile so that when no username is specified in commands, \
    it will choose the linked username.\n\
    If the value is set to `Link`, it will prompt you to authorize your account.\n\
    If `Unlink` is selected, you will be unlinked from the osu! profile.")]
    /// Specify whether you want to link to an osu! profile
    pub osu: Option<ConfigLink>,
    #[command(help = "With this option you can link to a twitch channel.\n\
    When you have both your osu! and twitch linked, are currently streaming, and anyone uses \
    the `recent score` command on your osu! username, it will try to retrieve the last VOD from your \
    twitch channel and link to a timestamp for the score.\n\
    If the value is set to `Link`, it will prompt you to authorize your account.\n\
    If `Unlink` is selected, you will be unlinked from the twitch channel.")]
    /// Specify whether you want to link to a twitch profile
    twitch: Option<ConfigLink>,
    #[command(help = "Always having to specify the `mode` option for any non-std \
    command can be pretty tedious.\nTo get around that, you can configure a mode here so \
    that when the `mode` option is not specified in commands, it will choose your config mode.")]
    /// Specify a gamemode (NOTE: Only use for non-std modes if you NEVER use std commands)
    mode: Option<ConfigGameMode>,
    /// What initial size should the profile command be?
    profile: Option<ProfileSize>,
    #[command(help = "Some embeds are pretty chunky and show too much data.\n\
    With this option you can make those embeds minimized by default.\n\
    Affected commands are: `compare score`, `recent score`, `recent simulate`, \
    and any command showing top scores when the `index` option is specified.")]
    /// What size should the recent, compare, simulate, ... commands be?
    embeds: Option<ConfigEmbeds>,
    /// Should the amount of retries be shown for the recent command?
    retries: Option<ShowHideOption>,
    /// Specify whether the recent command should show max or if-fc pp when minimized
    minimized_pp: Option<ConfigMinimizedPp>,
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
            ConfigGameMode::Osu => Some(GameMode::STD),
            ConfigGameMode::Taiko => Some(GameMode::TKO),
            ConfigGameMode::Catch => Some(GameMode::CTB),
            ConfigGameMode::Mania => Some(GameMode::MNA),
        }
    }
}

#[derive(CommandOption, CreateOption)]
pub enum ConfigEmbeds {
    #[option(name = "Initial maximized", value = "initial_max")]
    InitialMax,
    #[option(name = "Always maximized", value = "max")]
    AlwaysMax,
    #[option(name = "Always minimized", value = "min")]
    AlwaysMin,
}

impl From<ConfigEmbeds> for EmbedsSize {
    fn from(size: ConfigEmbeds) -> Self {
        match size {
            ConfigEmbeds::InitialMax => Self::InitialMaximized,
            ConfigEmbeds::AlwaysMax => Self::AlwaysMaximized,
            ConfigEmbeds::AlwaysMin => Self::AlwaysMinimized,
        }
    }
}

#[derive(CommandOption, CreateOption)]
pub enum ConfigMinimizedPp {
    #[option(name = "Max PP", value = "max")]
    MaxPp,
    #[option(name = "If FC", value = "if_fc")]
    IfFc,
}

impl From<ConfigMinimizedPp> for MinimizedPp {
    fn from(pp: ConfigMinimizedPp) -> Self {
        match pp {
            ConfigMinimizedPp::MaxPp => Self::Max,
            ConfigMinimizedPp::IfFc => Self::IfFc,
        }
    }
}

async fn slash_config(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Config::from_interaction(command.input_data())?;

    config(ctx, command, args).await
}

pub async fn config(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    config: Config,
) -> BotResult<()> {
    let Config {
        osu,
        twitch,
        mode,
        profile,
        embeds,
        retries,
        minimized_pp,
    } = config;

    let author = command.user_id()?;

    let mut config = match ctx.psql().get_user_config(author).await {
        Ok(Some(config)) => config,
        Ok(None) => UserConfig::default(),
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if let Some(pp) = minimized_pp {
        config.minimized_pp = Some(pp.into());
    }

    match mode {
        None => {}
        Some(ConfigGameMode::None) => config.mode = None,
        Some(ConfigGameMode::Osu) => config.mode = Some(GameMode::STD),
        Some(ConfigGameMode::Taiko) => config.mode = Some(GameMode::TKO),
        Some(ConfigGameMode::Catch) => config.mode = Some(GameMode::CTB),
        Some(ConfigGameMode::Mania) => config.mode = Some(GameMode::MNA),
    }

    if let Some(size) = profile {
        config.profile_size = Some(size);
    }

    if let Some(maximize) = embeds {
        config.embeds_size = Some(maximize.into());
    }

    if let Some(retries) = retries {
        config.show_retries = Some(matches!(retries, ShowHideOption::Show));
    }

    if let Some(ConfigLink::Unlink) = osu {
        config.osu.take();
    }

    if let Some(ConfigLink::Unlink) = twitch {
        config.twitch_id.take();
    }

    match (osu, twitch) {
        (Some(ConfigLink::Link), Some(ConfigLink::Link)) => {
            handle_both_links(&ctx, command, config).await
        }
        (Some(ConfigLink::Link), _) => handle_osu_link(&ctx, command, config).await,
        (_, Some(ConfigLink::Link)) => handle_twitch_link(&ctx, command, config).await,
        (_, _) => handle_no_links(&ctx, command, config).await,
    }
}

const MSG_BADE: &str = "Contact Badewanne3 if you encounter issues with the website";

fn osu_content(state: u8) -> String {
    let config = CONFIG.get().unwrap();

    format!(
        "{emote} [Click here](https://osu.ppy.sh/oauth/authorize?client_id={client_id}&\
        response_type=code&scope=identify&redirect_uri={url}/auth/osu&state={state}) \
        to authenticate your osu! profile",
        emote = Emote::Osu.text(),
        client_id = config.tokens.osu_client_id,
        url = config.server.external_url,
    )
}

fn twitch_content(state: u8) -> String {
    let config = CONFIG.get().unwrap();

    format!(
        "{emote} [Click here](https://id.twitch.tv/oauth2/authorize?client_id={client_id}\
        &response_type=code&scope=user:read:email&redirect_uri={url}/auth/twitch\
        &state={state}) to authenticate your twitch channel",
        emote = Emote::Twitch.text(),
        client_id = config.tokens.twitch_client_id,
        url = config.server.external_url,
    )
}

async fn handle_both_links(
    ctx: &Context,
    command: Box<ApplicationCommand>,
    mut config: UserConfig,
) -> BotResult<()> {
    let osu_fut = ctx.auth_standby.wait_for_osu();
    let twitch_fut = ctx.auth_standby.wait_for_twitch();

    let content = format!(
        "{}\n{}",
        osu_content(osu_fut.state),
        twitch_content(twitch_fut.state)
    );

    let embed = EmbedBuilder::new().description(content).footer(MSG_BADE);
    let builder = MessageBuilder::new().embed(embed);
    let fut = async { tokio::try_join!(osu_fut, twitch_fut) };
    let twitch_name;

    match handle_ephemeral(ctx, &command, builder, fut).await {
        Some(Ok((osu, twitch))) => {
            config.osu = Some(OsuData::User {
                user_id: osu.user_id,
                username: osu.username,
            });

            config.twitch_id = Some(twitch.user_id);
            twitch_name = Some(twitch.display_name);
        }
        Some(Err(why)) => return Err(why),
        None => return Ok(()),
    }

    let author = command.user()?;

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update(ctx, &builder).await?;

    Ok(())
}

async fn handle_twitch_link(
    ctx: &Context,
    command: Box<ApplicationCommand>,
    mut config: UserConfig,
) -> BotResult<()> {
    let fut = ctx.auth_standby.wait_for_twitch();

    let embed = EmbedBuilder::new()
        .description(twitch_content(fut.state))
        .footer(MSG_BADE);

    let builder = MessageBuilder::new().embed(embed);

    let twitch_name = match handle_ephemeral(ctx, &command, builder, fut).await {
        Some(Ok(user)) => {
            config.twitch_id = Some(user.user_id);

            Some(user.display_name)
        }
        Some(Err(why)) => return Err(why),
        None => return Ok(()),
    };

    let author = command.user()?;

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update(ctx, &builder).await?;

    Ok(())
}

async fn handle_osu_link(
    ctx: &Context,
    command: Box<ApplicationCommand>,
    mut config: UserConfig,
) -> BotResult<()> {
    let fut = ctx.auth_standby.wait_for_osu();

    let embed = EmbedBuilder::new()
        .description(osu_content(fut.state))
        .footer(MSG_BADE);

    let builder = MessageBuilder::new().embed(embed);

    config.osu = match handle_ephemeral(ctx, &command, builder, fut).await {
        Some(Ok(user)) => Some(OsuData::User {
            user_id: user.user_id,
            username: user.username,
        }),
        Some(Err(why)) => return Err(why),
        None => return Ok(()),
    };

    let author = command.user()?;
    let mut twitch_name = None;

    if let Some(user_id) = config.twitch_id {
        match ctx.client().get_twitch_user_by_id(user_id).await {
            Ok(Some(user)) => twitch_name = Some(user.display_name),
            Ok(None) => {
                debug!("No twitch user found for given id, remove from config");
                config.twitch_id.take();
            }
            Err(err) => {
                let _ = command.error(ctx, TWITCH_API_ISSUE).await;

                return Err(err.into());
            }
        }
    }

    if let Err(err) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(err);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update(ctx, &builder).await?;

    Ok(())
}

async fn handle_ephemeral<T>(
    ctx: &Context,
    command: &ApplicationCommand,
    builder: MessageBuilder<'_>,
    fut: impl Future<Output = Result<T, AuthenticationStandbyError>>,
) -> Option<BotResult<T>> {
    if let Err(err) = command.update(ctx, &builder).await {
        return Some(Err(err.into()));
    }

    let content = match fut.await {
        Ok(res) => return Some(Ok(res)),
        Err(AuthenticationStandbyError::Timeout) => "You did not authenticate in time",
        Err(AuthenticationStandbyError::Canceled) => GENERAL_ISSUE,
    };

    if let Err(err) = command.error(&ctx, content).await {
        return Some(Err(err.into()));
    }

    None
}

async fn handle_no_links(
    ctx: &Context,
    command: Box<ApplicationCommand>,
    mut config: UserConfig,
) -> BotResult<()> {
    let author = command.user()?;
    let mut twitch_name = None;

    if let Some(user_id) = config.twitch_id {
        match ctx.client().get_twitch_user_by_id(user_id).await {
            Ok(Some(user)) => twitch_name = Some(user.display_name),
            Ok(None) => {
                debug!("No twitch user found for given id, remove from config");
                config.twitch_id.take();
            }
            Err(why) => {
                let _ = command.error(ctx, TWITCH_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    if let Err(err) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(err);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update(ctx, &builder).await?;

    Ok(())
}
