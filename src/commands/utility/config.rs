use crate::{
    commands::{osu::ProfileSize, MyCommand, MyCommandOption},
    core::{server::AuthenticationStandbyError, CONFIG},
    database::UserConfig,
    embeds::{ConfigEmbed, EmbedBuilder, EmbedData},
    util::{
        constants::{
            common_literals::{CTB, MANIA, MODE, OSU, TAIKO},
            GENERAL_ISSUE, RED, TWITCH_API_ISSUE,
        },
        ApplicationCommandExt, Authored, Emote, MessageBuilder, MessageExt,
    },
    BotResult, CommandData, Context, Error,
};

use rosu_v2::prelude::GameMode;
use std::{future::Future, sync::Arc};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Deprecated command, use the slash command `/config` instead")]
async fn config(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, .. } => {
            let content = "This command is deprecated and no longer works.\n\
                Use the slash command `/config` instead.";

            return msg.error(&ctx, content).await;
        }
        CommandData::Interaction { command } => slash_config(ctx, *command).await,
    }
}

pub async fn config_(
    ctx: Arc<Context>,
    command: ApplicationCommand,
    args: ConfigArgs,
) -> BotResult<()> {
    let author = command.author().ok_or(Error::MissingInteractionAuthor)?;

    let ConfigArgs {
        mode,
        profile_size,
        embeds_maximized,
        show_retries,
        osu,
        twitch,
    } = args;

    let mut config = match ctx.psql().get_user_config(author.id).await {
        Ok(Some(config)) => config,
        Ok(None) => UserConfig::default(),
        Err(why) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    if let Some(mode) = mode {
        config.mode = mode;
    }

    if let Some(size) = profile_size {
        config.profile_size = Some(size);
    }

    if let Some(maximize) = embeds_maximized {
        config.embeds_maximized = maximize;
    }

    if let Some(retries) = show_retries {
        config.show_retries = retries;
    }

    if let Some(false) = osu {
        config.osu_username.take();
    }

    if let Some(false) = twitch {
        config.twitch_id.take();
    }

    match (osu.unwrap_or(false), twitch.unwrap_or(false)) {
        (false, false) => handle_no_links(&ctx, command, config).await,
        (true, false) => handle_osu_link(&ctx, command, config).await,
        (false, true) => handle_twitch_link(&ctx, command, config).await,
        (true, true) => handle_both_links(&ctx, command, config).await,
    }
}

fn osu_content(state: u8) -> String {
    let config = CONFIG.get().unwrap();

    format!(
        "{emote} [Click here](https://osu.ppy.sh/oauth/authorize?client_id={client_id}&\
        response_type=code&scope=identify&redirect_uri={url}/auth/osu&state={state}) \
        to authenticate your osu! profile",
        emote = Emote::Osu.text(),
        client_id = config.tokens.osu_client_id,
        url = config.server.external_url,
        state = state,
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
        state = state,
    )
}

async fn handle_both_links(
    ctx: &Context,
    command: ApplicationCommand,
    mut config: UserConfig,
) -> BotResult<()> {
    let osu_fut = ctx.auth_standby.wait_for_osu();
    let twitch_fut = ctx.auth_standby.wait_for_twitch();

    let content = format!(
        "{}\n{}",
        osu_content(osu_fut.state),
        twitch_content(twitch_fut.state)
    );

    let builder = MessageBuilder::new().embed(content);
    let fut = async { tokio::try_join!(osu_fut, twitch_fut) };
    let twitch_name;

    match handle_ephemeral(ctx, &command, builder, fut).await {
        Some(Ok((osu, twitch))) => {
            config.osu_username = Some(osu.username.into());
            config.twitch_id = Some(twitch.user_id);
            twitch_name = Some(twitch.display_name);
        }
        Some(Err(why)) => return Err(why),
        None => return Ok(()),
    }

    let author = command.author().ok_or(Error::MissingInteractionAuthor)?;

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update_message(ctx, builder).await?;

    Ok(())
}

async fn handle_twitch_link(
    ctx: &Context,
    command: ApplicationCommand,
    mut config: UserConfig,
) -> BotResult<()> {
    let fut = ctx.auth_standby.wait_for_twitch();
    let builder = MessageBuilder::new().embed(twitch_content(fut.state));
    let twitch_name;

    match handle_ephemeral(ctx, &command, builder, fut).await {
        Some(Ok(user)) => {
            config.twitch_id = Some(user.user_id);
            twitch_name = Some(user.display_name);
        }
        Some(Err(why)) => return Err(why),
        None => return Ok(()),
    }

    let author = command.author().ok_or(Error::MissingInteractionAuthor)?;

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update_message(ctx, builder).await?;

    Ok(())
}

async fn handle_osu_link(
    ctx: &Context,
    command: ApplicationCommand,
    mut config: UserConfig,
) -> BotResult<()> {
    let fut = ctx.auth_standby.wait_for_osu();
    let builder = MessageBuilder::new().embed(osu_content(fut.state));

    match handle_ephemeral(ctx, &command, builder, fut).await {
        Some(Ok(user)) => config.osu_username = Some(user.username.into()),
        Some(Err(why)) => return Err(why),
        None => return Ok(()),
    }

    let author = command.author().ok_or(Error::MissingInteractionAuthor)?;
    let mut twitch_name = None;

    if let Some(user_id) = config.twitch_id {
        match ctx.clients.twitch.get_user_by_id(user_id).await {
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

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.update_message(ctx, builder).await?;

    Ok(())
}

async fn handle_ephemeral<T>(
    ctx: &Context,
    command: &ApplicationCommand,
    builder: MessageBuilder<'_>,
    fut: impl Future<Output = Result<T, AuthenticationStandbyError>>,
) -> Option<BotResult<T>> {
    if let Err(why) = command.create_message(ctx, builder).await {
        return Some(Err(why));
    }

    let content = match fut.await {
        Ok(res) => return Some(Ok(res)),
        Err(AuthenticationStandbyError::Timeout) => "You did not authenticate in time",
        Err(AuthenticationStandbyError::Canceled) => GENERAL_ISSUE,
    };

    let builder =
        MessageBuilder::new().embed(EmbedBuilder::new().color(RED).description(content).build());

    if let Err(why) = command.update_message(ctx, builder).await {
        return Some(Err(why));
    }

    None
}

async fn handle_no_links(
    ctx: &Context,
    command: ApplicationCommand,
    mut config: UserConfig,
) -> BotResult<()> {
    let author = command.author().ok_or(Error::MissingInteractionAuthor)?;
    let mut twitch_name = None;

    if let Some(user_id) = config.twitch_id {
        match ctx.clients.twitch.get_user_by_id(user_id).await {
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

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = command.error(ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config, twitch_name);
    let builder = embed_data.into_builder().build().into();
    command.create_message(ctx, builder).await?;

    Ok(())
}

#[derive(Default)]
pub struct ConfigArgs {
    embeds_maximized: Option<bool>,
    mode: Option<Option<GameMode>>,
    profile_size: Option<ProfileSize>,
    show_retries: Option<bool>,
    pub osu: Option<bool>,
    pub twitch: Option<bool>,
}

impl ConfigArgs {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut mode = None;
        let mut profile_size = None;
        let mut embeds_maximized = None;
        let mut show_retries = None;
        let mut osu = None;
        let mut twitch = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    MODE => {
                        mode = match value.as_str() {
                            "none" => Some(None),
                            OSU => Some(Some(GameMode::STD)),
                            TAIKO => Some(Some(GameMode::TKO)),
                            CTB => Some(Some(GameMode::CTB)),
                            MANIA => Some(Some(GameMode::MNA)),
                            _ => bail_cmd_option!("config mode", string, value),
                        }
                    }
                    "profile" => match value.as_str() {
                        "compact" => profile_size = Some(ProfileSize::Compact),
                        "medium" => profile_size = Some(ProfileSize::Medium),
                        "full" => profile_size = Some(ProfileSize::Full),
                        _ => bail_cmd_option!("config profile", string, value),
                    },
                    "embeds" => match value.as_str() {
                        "maximized" => embeds_maximized = Some(true),
                        "minimized" => embeds_maximized = Some(false),
                        _ => bail_cmd_option!("config embeds", string, value),
                    },
                    "retries" => match value.as_str() {
                        "show" => show_retries = Some(true),
                        "hide" => show_retries = Some(false),
                        _ => bail_cmd_option!("config retries", string, value),
                    },
                    _ => bail_cmd_option!("config", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("config", integer, name)
                }
                CommandDataOption::Boolean { name, value } => match name.as_str() {
                    OSU => osu = Some(value),
                    "twitch" => twitch = Some(value),
                    _ => bail_cmd_option!("config", boolean, name),
                },
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("config", subcommand, name)
                }
            }
        }

        let args = Self {
            mode,
            profile_size,
            embeds_maximized,
            show_retries,
            osu,
            twitch,
        };

        Ok(args)
    }
}

pub async fn slash_config(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let args = ConfigArgs::slash(&mut command)?;

    config_(ctx, command, args).await
}

pub fn define_config() -> MyCommand {
    let osu_description =
        "Specify whether you want to link to an osu! profile (choose `false` to unlink)";

    let osu_help = "Most osu! commands require a specified username to work.\n\
        Since using a command is most commonly intended for your own profile, you can link \
        your discord with an osu! profile so that when no username is specified in commands, \
        it will choose the linked username.\n\
        If the value is set to `True`, it will prompt you to authorize your account.\n\
        If `False` is selected, you will be unlinked from the osu! profile.";

    let osu = MyCommandOption::builder(OSU, osu_description)
        .help(osu_help)
        .boolean(false);

    let twitch_description =
        "Specify whether you want to link to a twitch profile (choose `false` to unlink)";

    let twitch_help = "With this option you can link to a twitch channel.\n\
        When you have both your osu! and twitch linked, are currently streaming, and anyone uses \
        the `recent score` command on your osu! username, it will try to retrieve the last VOD from your \
        twitch channel and link to a timestamp for the score.\n\
        If the value is set to `True`, it will prompt you to authorize your account.\n\
        If `False` is selected, you will be unlinked from the twitch channel.";

    let twitch = MyCommandOption::builder("twitch", twitch_description)
        .help(twitch_help)
        .boolean(false);

    let mode_description =
        "Specify a gamemode (NOTE: Only use for non-std modes if you NEVER use std commands)";

    let mode_help = "Always having to specify the `mode` option for any non-std \
        command can be pretty tedious.\nTo get around that, you can configure a mode here so \
        that when the `mode` option is not specified in commands, it will choose your config mode.";

    let mode_choices = vec![
        CommandOptionChoice::String {
            name: "none".to_owned(),
            value: "none".to_owned(),
        },
        CommandOptionChoice::String {
            name: OSU.to_owned(),
            value: OSU.to_owned(),
        },
        CommandOptionChoice::String {
            name: TAIKO.to_owned(),
            value: TAIKO.to_owned(),
        },
        CommandOptionChoice::String {
            name: CTB.to_owned(),
            value: CTB.to_owned(),
        },
        CommandOptionChoice::String {
            name: MANIA.to_owned(),
            value: MANIA.to_owned(),
        },
    ];

    let mode = MyCommandOption::builder(MODE, mode_description)
        .help(mode_help)
        .string(mode_choices, false);

    let profile_description = "What initial size should the profile command be?";

    let profile_choices = vec![
        CommandOptionChoice::String {
            name: "compact".to_owned(),
            value: "compact".to_owned(),
        },
        CommandOptionChoice::String {
            name: "medium".to_owned(),
            value: "medium".to_owned(),
        },
        CommandOptionChoice::String {
            name: "full".to_owned(),
            value: "full".to_owned(),
        },
    ];

    let profile =
        MyCommandOption::builder("profile", profile_description).string(profile_choices, false);

    let embeds_description =
        "What initial size should the recent, compare, simulate, ... commands be?";

    let embeds_help = "Some embeds are pretty chunky and show too much data.\n\
        With this option you can make those embeds minimized by default.\n\
        Affected commands are: `compare score`, `recent score`, `recent simulate`, \
        and any command showing top scores when the `index` option is specified.";

    let embeds_choices = vec![
        CommandOptionChoice::String {
            name: "maximized".to_owned(),
            value: "maximized".to_owned(),
        },
        CommandOptionChoice::String {
            name: "minimized".to_owned(),
            value: "minimized".to_owned(),
        },
    ];

    let embeds = MyCommandOption::builder("embeds", embeds_description)
        .help(embeds_help)
        .string(embeds_choices, false);

    let retries_description = "Should the amount of retries be shown for the `recent` command?";

    let retries_choices = vec![
        CommandOptionChoice::String {
            name: "show".to_owned(),
            value: "show".to_owned(),
        },
        CommandOptionChoice::String {
            name: "hide".to_owned(),
            value: "hide".to_owned(),
        },
    ];

    let retries =
        MyCommandOption::builder("retries", retries_description).string(retries_choices, false);

    MyCommand::new("config", "Adjust your default configuration for commands")
        .options(vec![osu, twitch, mode, profile, embeds, retries])
}
