use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{constants::OSU_API_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found};
use crate::{
    core::{commands::CommandOrigin, ContextExt},
    embeds::{EmbedData, RatioEmbed},
    manager::redis::osu::UserArgs,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(
    name = "ratios",
    desc = "Ratio related stats about a user's mania top100",
    help = "The \"ratio\" of a mania score is generally considered to be `n320/n300` \
    (or sometimes `n320/everything else`).\n\n\
    How to read the embed:\n\
    The first column defines how the top scores are split up based on their accuracy.\n\
    E.g. `>90%` will only include top scores that have more than 90% accuracy.\n\
    The second column tells how many scores are in the corresponding accuracy row.\n\
    For the third column, it calculates the ratio of all scores in that row and displays their average.\n\
    The fourth column shows the average percentual miss amount for scores in the corresponding row."
)]
pub struct Ratios<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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
async fn prefix_ratios(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
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

async fn slash_ratios(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Ratios::from_interaction(command.input_data())?;

    ratios(ctx, (&mut command).into(), args).await
}

async fn ratios(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Ratios<'_>) -> Result<()> {
    let owner = orig.user_id()?;
    let config = ctx.user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let legacy_scores = match config.legacy_scores {
        Some(legacy_scores) => legacy_scores,
        None => match orig.guild_id() {
            Some(guild_id) => ctx
                .guild_config()
                .peek(guild_id, |config| config.legacy_scores)
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id)
        .await
        .mode(GameMode::Mania);

    let scores_fut = ctx
        .osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    // Accumulate all necessary data
    let embed_data = RatioEmbed::new(&user, scores);

    let content = format!(
        "Average ratios of `{}`'s top 100 in mania:",
        user.username()
    );

    // Creating the embed
    let embed = embed_data.build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}
