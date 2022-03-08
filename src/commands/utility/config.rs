use crate::{
    commands::{osu::ProfileSize, MyCommand, MyCommandOption},
    core::CONFIG,
    database::{EmbedsSize, MinimizedPp, OsuData, UserConfig},
    embeds::{ConfigEmbed, EmbedBuilder, EmbedData},
    server::AuthenticationStandbyError,
    util::{
        constants::{
            common_literals::{CTB, MANIA, MODE, OSU, PROFILE, TAIKO},
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
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

const MSG_BADE: &str = "Contact Badewanne3 if you encounter issues with the website";

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
        embeds_size,
        minimized_pp,
        mode,
        profile_size,
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

    if let Some(pp) = minimized_pp {
        config.minimized_pp = Some(pp);
    }

    if let Some(mode) = mode {
        config.mode = mode;
    }

    if let Some(size) = profile_size {
        config.profile_size = Some(size);
    }

    if let Some(maximize) = embeds_size {
        config.embeds_size = Some(maximize);
    }

    if let Some(retries) = show_retries {
        config.show_retries = Some(retries);
    }

    if let Some(false) = osu {
        config.osu.take();
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

    let author = command.author().ok_or(Error::MissingInteractionAuthor)?;
    let mut twitch_name = None;

    if let Some(user_id) = config.twitch_id {
        match ctx.clients.custom.get_twitch_user_by_id(user_id).await {
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
        match ctx.clients.custom.get_twitch_user_by_id(user_id).await {
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
    embeds_size: Option<EmbedsSize>,
    minimized_pp: Option<MinimizedPp>,
    mode: Option<Option<GameMode>>,
    profile_size: Option<ProfileSize>,
    show_retries: Option<bool>,
    pub osu: Option<bool>,
    pub twitch: Option<bool>,
}

impl ConfigArgs {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut minimized_pp = None;
        let mut mode = None;
        let mut profile_size = None;
        let mut embeds_size = None;
        let mut show_retries = None;
        let mut osu = None;
        let mut twitch = None;

        for option in command.yoink_options() {
            if let CommandOptionValue::String(value) = option.value {
                match option.name.as_str() {
                    OSU => osu = Some(value == "link"),
                    "twitch" => twitch = Some(value == "link"),
                    "minimized_pp" => match value.as_str() {
                        "max" => minimized_pp = Some(MinimizedPp::Max),
                        "if_fc" => minimized_pp = Some(MinimizedPp::IfFc),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    MODE => {
                        mode = match value.as_str() {
                            "none" => Some(None),
                            OSU => Some(Some(GameMode::STD)),
                            TAIKO => Some(Some(GameMode::TKO)),
                            CTB => Some(Some(GameMode::CTB)),
                            MANIA => Some(Some(GameMode::MNA)),
                            _ => return Err(Error::InvalidCommandOptions),
                        }
                    }
                    PROFILE => match value.as_str() {
                        "compact" => profile_size = Some(ProfileSize::Compact),
                        "medium" => profile_size = Some(ProfileSize::Medium),
                        "full" => profile_size = Some(ProfileSize::Full),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "embeds" => match value.as_str() {
                        "initial_maximized" => embeds_size = Some(EmbedsSize::InitialMaximized),
                        "maximized" => embeds_size = Some(EmbedsSize::AlwaysMaximized),
                        "minimized" => embeds_size = Some(EmbedsSize::AlwaysMinimized),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "retries" => match value.as_str() {
                        "show" => show_retries = Some(true),
                        "hide" => show_retries = Some(false),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                }
            } else {
                return Err(Error::InvalidCommandOptions);
            }
        }

        let args = Self {
            minimized_pp,
            mode,
            profile_size,
            embeds_size,
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

fn link_options() -> Vec<CommandOptionChoice> {
    let link = CommandOptionChoice::String {
        name: "Link".to_owned(),
        value: "link".to_owned(),
    };

    let unlink = CommandOptionChoice::String {
        name: "Unlink".to_owned(),
        value: "unlink".to_owned(),
    };

    vec![link, unlink]
}

pub fn define_config() -> MyCommand {
    let osu_description = "Specify whether you want to link to an osu! profile";

    let osu_help = "Most osu! commands require a specified username to work.\n\
        Since using a command is most commonly intended for your own profile, you can link \
        your discord with an osu! profile so that when no username is specified in commands, \
        it will choose the linked username.\n\
        If the value is set to `Link`, it will prompt you to authorize your account.\n\
        If `Unlink` is selected, you will be unlinked from the osu! profile.";

    let osu = MyCommandOption::builder(OSU, osu_description)
        .help(osu_help)
        .string(link_options(), false);

    let twitch_description = "Specify whether you want to link to a twitch profile";

    let twitch_help = "With this option you can link to a twitch channel.\n\
        When you have both your osu! and twitch linked, are currently streaming, and anyone uses \
        the `recent score` command on your osu! username, it will try to retrieve the last VOD from your \
        twitch channel and link to a timestamp for the score.\n\
        If the value is set to `Link`, it will prompt you to authorize your account.\n\
        If `Unlink` is selected, you will be unlinked from the twitch channel.";

    let twitch = MyCommandOption::builder("twitch", twitch_description)
        .help(twitch_help)
        .string(link_options(), false);

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
        MyCommandOption::builder(PROFILE, profile_description).string(profile_choices, false);

    let embeds_description = "What size should the recent, compare, simulate, ... commands be?";

    let embeds_help = "Some embeds are pretty chunky and show too much data.\n\
        With this option you can make those embeds minimized by default.\n\
        Affected commands are: `compare score`, `recent score`, `recent simulate`, \
        and any command showing top scores when the `index` option is specified.";

    let embeds_choices = vec![
        CommandOptionChoice::String {
            name: "Initial maximized".to_owned(),
            value: "initial_maximized".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Always maximized".to_owned(),
            value: "maximized".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Always minimized".to_owned(),
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

    let minimized_pp_description =
        "Specify whether the recent command should show max or if-fc pp when minimized";

    let minimized_pp_choices = vec![
        CommandOptionChoice::String {
            name: "Max PP".to_owned(),
            value: "max".to_owned(),
        },
        CommandOptionChoice::String {
            name: "If FC".to_owned(),
            value: "if_fc".to_owned(),
        },
    ];

    let minimized_pp = MyCommandOption::builder("minimized_pp", minimized_pp_description)
        .string(minimized_pp_choices, false);

    MyCommand::new("config", "Adjust your default configuration for commands").options(vec![
        osu,
        twitch,
        mode,
        profile,
        embeds,
        retries,
        minimized_pp,
    ])
}
