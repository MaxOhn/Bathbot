use std::sync::Arc;

use rosu_v2::prelude::{GameMode, OsuError, Username};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    commands::{
        check_user_mention,
        osu::{option_discord, option_name},
        parse_discord, DoubleResultCow, MyCommand,
    },
    database::OsuData,
    embeds::{EmbedData, RatioEmbed},
    error::Error,
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use super::{ScoreArgs, UserArgs};

#[command]
#[short_desc("Ratio related stats about a user's top100")]
#[long_desc(
    "Calculate the average ratios of a user's top100.\n\
    If the command was used before on the given osu name, \
    I will also compare the current results with the ones from last time \
    if they've changed since."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("ratio")]
async fn ratios(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match check_user_mention(&ctx, arg).await {
                    Ok(Ok(osu)) => Some(osu.into_username()),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.psql().get_user_osu(msg.author.id).await {
                    Ok(osu) => osu.map(OsuData::into_username),
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            _ratios(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => slash_ratio(ctx, *command).await,
    }
}

async fn _ratios(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), GameMode::MNA);
    let score_args = ScoreArgs::top(100);

    let (mut user, mut scores) =
        match super::get_user_and_scores(&ctx, user_args, &score_args).await {
            Ok((user, scores)) => (user, scores),
            Err(OsuError::NotFound) => {
                let content = format!("User `{name}` was not found");

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        };

    // Overwrite default mode
    user.mode = GameMode::MNA;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Accumulate all necessary data
    let embed_data = RatioEmbed::new(user, scores);
    let content = format!("Average ratios of `{name}`'s top 100 in mania:");

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

async fn parse_username(
    ctx: &Context,
    command: &mut ApplicationCommand,
) -> DoubleResultCow<Option<Username>> {
    let mut osu = None;

    for option in command.yoink_options() {
        match option.value {
            CommandOptionValue::String(value) => match option.name.as_str() {
                NAME => osu = Some(value.into()),
                _ => return Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::User(value) => match option.name.as_str() {
                DISCORD => match parse_discord(ctx, value).await? {
                    Ok(osu_) => osu = Some(osu_),
                    Err(content) => return Ok(Err(content)),
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    let osu = match osu {
        Some(osu) => Some(osu),
        None => ctx.psql().get_user_osu(command.user_id()?).await?,
    };

    Ok(Ok(osu.map(OsuData::into_username)))
}

pub async fn slash_ratio(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match parse_username(&ctx, &mut command).await? {
        Ok(name) => _ratios(ctx, command.into(), name).await,
        Err(content) => return command.error(&ctx, content).await,
    }
}

pub fn define_ratios() -> MyCommand {
    let name = option_name();
    let discord = option_discord();

    let help = "The \"ratio\" of a mania score is generally considered to be `n320/n300` \
        (or sometimes `n320/everything else`).\n\n\
        How to read the embed:\n\
        The first column defines how the top scores are split up based on their accuracy.\n\
        E.g. `>90%` will only include top scores that have more than 90% accuracy.\n\
        The second column tells how many scores are in the corresponding accuracy row.\n\
        For the third column, it calculates the ratio of all scores in that row and displays their average.\n\
        The fourth column shows the average percentual miss amount for scores in the corresponding row.";

    MyCommand::new("ratios", "Ratio related stats about a user's mania top100")
        .help(help)
        .options(vec![name, discord])
}
