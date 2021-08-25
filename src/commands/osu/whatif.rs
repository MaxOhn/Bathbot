use crate::{
    custom_client::RankParam,
    embeds::{EmbedData, WhatIfEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::application::{
    command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

async fn _whatif(ctx: Arc<Context>, data: CommandData<'_>, args: WhatIfArgs) -> BotResult<()> {
    let WhatIfArgs { name, mut mode, pp } = args;

    let author_id = data.author()?.id;

    mode = match ctx.user_config(author_id).await {
        Ok(config) => config.mode(mode),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    if pp < 0.0 {
        return data.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
        return data.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, Some(mode));

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    let whatif_data = if scores.is_empty() {
        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                unwind_error!(warn, why, "Error while getting rank pp: {}");

                None
            }
        };

        WhatIfData::NoScores { rank }
    } else if pp < scores.last().and_then(|s| s.pp).unwrap_or(0.0) {
        WhatIfData::NonTop100
    } else {
        let actual: f32 = scores
            .iter()
            .filter_map(|score| score.weight)
            .map(|weight| weight.pp)
            .sum();

        let bonus_pp = user.statistics.as_ref().unwrap().pp - actual;
        let mut potential = 0.0;
        let mut used = false;
        let mut new_pos = scores.len();
        let mut factor = 1.0;

        let pp_iter = scores
            .iter()
            .take(scores.len() - 1)
            .filter_map(|score| score.pp)
            .enumerate();

        for (i, pp_value) in pp_iter {
            if !used && pp_value < pp {
                used = true;
                potential += pp * factor;
                factor *= 0.95;
                new_pos = i + 1;
            }

            potential += pp_value * factor;
            factor *= 0.95;
        }

        if !used {
            potential += pp * factor;
        };

        let new_pp = potential;
        let max_pp = scores.first().and_then(|s| s.pp).unwrap_or(0.0);

        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(new_pp + bonus_pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                unwind_error!(warn, why, "Error while getting rank pp: {}");
                None
            }
        };

        WhatIfData::Top100 {
            bonus_pp,
            new_pp,
            new_pos,
            max_pp,
            rank,
        }
    };

    // Sending the embed
    let builder = WhatIfEmbed::new(user, pp, whatif_data)
        .into_builder()
        .build()
        .into();

    data.create_message(&ctx, builder).await?;

    Ok(())
}

#[command]
#[short_desc("Display the impact of a new X pp score for a user")]
#[long_desc(
    "Calculate the gain in pp if the user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wi")]
pub async fn whatif(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(whatif_args) => {
                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the impact of a new X pp score for a mania user")]
#[long_desc(
    "Calculate the gain in pp if the mania user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wim")]
pub async fn whatifmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(whatif_args) => {
                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the impact of a new X pp score for a taiko user")]
#[long_desc(
    "Calculate the gain in pp if the taiko user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wit")]
pub async fn whatiftaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(whatif_args) => {
                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, command).await,
    }
}

#[command]
#[short_desc("Display the impact of a new X pp score for a ctb user")]
#[long_desc(
    "Calculate the gain in pp if the ctb user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wic")]
pub async fn whatifctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match WhatIfArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(whatif_args) => {
                    _whatif(ctx, CommandData::Message { msg, args, num }, whatif_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_whatif(ctx, command).await,
    }
}

pub enum WhatIfData {
    NonTop100,
    NoScores {
        rank: Option<u32>,
    },
    Top100 {
        bonus_pp: f32,
        new_pp: f32,
        new_pos: usize,
        max_pp: f32,
        rank: Option<u32>,
    },
}

struct WhatIfArgs {
    name: Option<Name>,
    mode: GameMode,
    pp: f32,
}

impl WhatIfArgs {
    fn args(ctx: &Context, args: &mut Args, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
        let mut pp = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => pp = Some(num),
                Err(_) => name = Some(Args::try_link_name(ctx, arg)?),
            }
        }

        let pp = pp.ok_or("You need to provide a decimal number")?;

        Ok(Self { name, pp, mode })
    }

    fn slash(ctx: &Context, command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;
        let mut pp = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => mode = parse_mode_option!(value, "whatif"),
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "whatif"),
                    _ => bail_cmd_option!("whatif", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("whatif", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("whatif", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("whatif", subcommand, name)
                }
            }
        }

        let args = Self {
            pp: pp.ok_or(Error::InvalidCommandOptions)?,
            name: username,
            mode: mode.unwrap_or(GameMode::STD),
        };

        Ok(Ok(args))
    }
}

pub async fn slash_whatif(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match WhatIfArgs::slash(&ctx, &mut command)? {
        Ok(args) => _whatif(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_whatif_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "whatif".to_owned(),
        default_permission: None,
        description: "Display the impact of a new X pp score for a user".to_owned(),
        id: None,
        options: vec![
            // TODO
            // CommandOption::Number(ChoiceCommandOptionData {
            //     choices: vec![],
            //     description: "Specify a pp amount".to_owned(),
            //     name: "pp".to_owned(),
            //     required: true,
            // }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: super::mode_choices(),
                description: "Specify the gamemode".to_owned(),
                name: "mode".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a username".to_owned(),
                name: "name".to_owned(),
                required: false,
            }),
            CommandOption::User(BaseCommandOptionData {
                description: "Specify a linked discord user".to_owned(),
                name: "discord".to_owned(),
                required: false,
            }),
        ],
    }
}
