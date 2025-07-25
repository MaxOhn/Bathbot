use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand, command};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{MessageBuilder, constants::GENERAL_ISSUE, matcher};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{Id, marker::UserMarker};

use super::{require_link, user_not_found};
use crate::{
    Context,
    commands::{DISCORD_OPTION_DESC, DISCORD_OPTION_HELP},
    core::commands::CommandOrigin,
    embeds::{EmbedData, RatioEmbed},
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

const RATIOS_DESC: &str = "Ratio related stats about a user's mania top100";

#[derive(CommandModel, CreateCommand, Default, HasName, SlashCommand)]
#[command(
    name = "ratios",
    desc = RATIOS_DESC,
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
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[command]
#[desc(RATIOS_DESC)]
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
async fn prefix_ratios(msg: &Message, mut args: Args<'_>) -> Result<()> {
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

    ratios(msg.into(), args).await
}

async fn slash_ratios(mut command: InteractionCommand) -> Result<()> {
    let args = Ratios::from_interaction(command.input_data())?;

    ratios((&mut command).into(), args).await
}

async fn ratios(orig: CommandOrigin<'_>, args: Ratios<'_>) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .is_some_and(ScoreData::is_legacy),
            None => false,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&user_id, GameMode::Mania).await;

    let scores_fut = Context::osu_scores()
        .top(100, legacy_scores)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    // Accumulate all necessary data
    let embed_data = RatioEmbed::new(&user, scores);

    let content = format!(
        "Average ratios of `{}`'s top 100 in mania:",
        user.username.as_str()
    );

    // Creating the embed
    let embed = embed_data.build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}
