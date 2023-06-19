use std::{sync::Arc, time::Duration};

use bathbot_macros::SlashCommand;
use bathbot_util::{constants::OSU_API_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::prelude::{MatchScore, OsuError, Team};
use tokio::time::interval;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use super::retrieve_previous;
use crate::{
    active::{impls::MatchComparePagination, ActiveMessages},
    core::Context,
    util::{interaction::InteractionCommand, Authored, ChannelExt, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "matchcompare", desc = "Compare two multiplayer matches")]
#[bucket(MatchCompare)]
pub struct MatchCompare {
    #[command(desc = "Specify the first match url or match id")]
    match_url_1: String,
    #[command(desc = "Specify the second match url or match id")]
    match_url_2: String,
    #[command(desc = "Specify if the response should be paginated or all at once")]
    output: Option<MatchCompareOutput>,
    #[command(desc = "Specify if it should show comparisons between players or teams")]
    comparison: Option<MatchCompareComparison>,
}

#[derive(CommandOption, CreateOption)]
pub enum MatchCompareOutput {
    #[option(name = "Full", value = "full")]
    Full,
    #[option(name = "Paginated", value = "paginated")]
    Paginated,
}

impl Default for MatchCompareOutput {
    fn default() -> Self {
        Self::Paginated
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum MatchCompareComparison {
    #[option(name = "Compare players", value = "players")]
    Players,
    #[option(name = "Compare teams", value = "teams")]
    Teams,
    #[option(name = "Compare both", value = "both")]
    Both,
}

impl Default for MatchCompareComparison {
    fn default() -> Self {
        Self::Players
    }
}

async fn slash_matchcompare(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = MatchCompare::from_interaction(command.input_data())?;

    matchcompare(ctx, command, args).await
}

async fn matchcompare(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: MatchCompare,
) -> Result<()> {
    let MatchCompare {
        match_url_1,
        match_url_2,
        output,
        comparison,
    } = args;

    let match_id1 = match matcher::get_osu_match_id(&match_url_1) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let match_id2 = match matcher::get_osu_match_id(&match_url_2) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    if match_id1 == match_id2 {
        let content = "Trying to compare a match with itself huh";
        command.error(&ctx, content).await?;

        return Ok(());
    }

    let match_fut1 = ctx.osu().osu_match(match_id1);
    let match_fut2 = ctx.osu().osu_match(match_id2);

    let output = output.unwrap_or_default();
    let comparison = comparison.unwrap_or_default();

    let pagination = match tokio::try_join!(match_fut1, match_fut2) {
        Ok((mut match1, mut match2)) => {
            let previous_fut_1 = retrieve_previous(&mut match1, ctx.osu());
            let previous_fut_2 = retrieve_previous(&mut match2, ctx.osu());

            if let Err(err) = tokio::try_join!(previous_fut_1, previous_fut_2) {
                let _ = command.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err)
                    .wrap_err("Failed to get history of at least one of the matches");

                return Err(report);
            }

            let owner = command.user_id()?;

            MatchComparePagination::new(&mut match1, &mut match2, comparison, owner)
        }
        Err(OsuError::NotFound) => {
            let content = "At least one of the two given matches was not found";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(OsuError::Response { status, .. }) if status == 401 => {
            let content =
                "I can't access at least one of the two matches because it was set as private";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get at least one of the matches");

            return Err(report);
        }
    };

    match output {
        MatchCompareOutput::Full => {
            let mut embeds = pagination.into_embeds().into_iter();

            if let Some(embed) = embeds.next() {
                let builder = MessageBuilder::new().embed(embed);
                command.update(&ctx, builder).await?;

                let mut interval = interval(Duration::from_secs(1));
                interval.tick().await;

                for embed in embeds {
                    interval.tick().await;

                    command
                        .channel_id
                        .create_message(&ctx, embed.into(), command.permissions)
                        .await?;
                }
            }
        }
        MatchCompareOutput::Paginated => {
            ActiveMessages::builder(pagination)
                .start_by_update(true)
                .begin(ctx, &mut command)
                .await?;
        }
    }

    Ok(())
}

trait HasScore {
    fn team(&self) -> Team;
    fn score(&self) -> u32;
}

impl HasScore for MatchScore {
    fn team(&self) -> Team {
        self.team
    }

    fn score(&self) -> u32 {
        self.score
    }
}
