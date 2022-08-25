use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, RatioEmbed},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder, constants::OSU_API_ISSUE, interaction::InteractionCommand,
        matcher, InteractionCommandExt,
    },
    BotResult, Context,
};

use super::{require_link, ScoreArgs, UserArgs};

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(
    name = "ratios",
    help = "The \"ratio\" of a mania score is generally considered to be `n320/n300` \
    (or sometimes `n320/everything else`).\n\n\
    How to read the embed:\n\
    The first column defines how the top scores are split up based on their accuracy.\n\
    E.g. `>90%` will only include top scores that have more than 90% accuracy.\n\
    The second column tells how many scores are in the corresponding accuracy row.\n\
    For the third column, it calculates the ratio of all scores in that row and displays their average.\n\
    The fourth column shows the average percentual miss amount for scores in the corresponding row."
)]
/// Ratio related stats about a user's mania top100
pub struct Ratios<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[command]
#[desc("Ratio related stats about a user's top100")]
#[help(
    "Calculate the average ratios of a user's top100.\n\
    If the command was used before on the given osu name, \
    I will also compare the current results with the ones from last time \
    if they've changed since."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("ratio")]
#[group(Mania)]
async fn prefix_ratios(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => Ratios {
                name: None,
                discord: Some(id),
            },
            None => Ratios {
                name: Some(Cow::Borrowed(arg)),
                discord: None,
            },
        },
        None => Ratios::default(),
    };

    ratios(ctx, msg.into(), args).await
}

async fn slash_ratios(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    let args = Ratios::from_interaction(command.input_data())?;

    ratios(ctx, (&mut command).into(), args).await
}

async fn ratios(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Ratios<'_>) -> BotResult<()> {
    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.user_config(orig.user_id()?).await?.into_username() {
            Some(name) => name,
            None => return require_link(&ctx, &orig).await,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), GameMode::Mania);
    let score_args = ScoreArgs::top(100);

    let (mut user, mut scores) =
        match super::get_user_and_scores(&ctx, user_args, &score_args).await {
            Ok((user, scores)) => (user, scores),
            Err(OsuError::NotFound) => {
                let content = format!("User `{name}` was not found");

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
        };

    // Overwrite default mode
    user.mode = GameMode::Mania;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Accumulate all necessary data
    let embed_data = RatioEmbed::new(user, scores);
    let content = format!("Average ratios of `{name}`'s top 100 in mania:");

    // Creating the embed
    let embed = embed_data.build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}
