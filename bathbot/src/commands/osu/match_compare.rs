use std::time::Duration;

use bathbot_macros::SlashCommand;
use bathbot_util::{Authored, MessageBuilder, constants::OSU_API_ISSUE, matcher};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;
use tokio::time::interval;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use super::retrieve_previous;
use crate::{
    active::{ActiveMessages, impls::MatchComparePagination},
    core::Context,
    util::{ChannelExt, InteractionCommandExt, interaction::InteractionCommand},
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

#[derive(CommandOption, CreateOption, Default)]
pub enum MatchCompareOutput {
    #[option(name = "Full", value = "full")]
    Full,
    #[option(name = "Paginated", value = "paginated")]
    #[default]
    Paginated,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Default)]
pub enum MatchCompareComparison {
    #[option(name = "Compare players", value = "players")]
    #[default]
    Players,
    #[option(name = "Compare teams", value = "teams")]
    Teams,
    #[option(name = "Compare both", value = "both")]
    Both,
}

async fn slash_matchcompare(mut command: InteractionCommand) -> Result<()> {
    let args = MatchCompare::from_interaction(command.input_data())?;

    matchcompare(command, args).await
}

async fn matchcompare(mut command: InteractionCommand, args: MatchCompare) -> Result<()> {
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
            command.error(content).await?;

            return Ok(());
        }
    };

    let match_id2 = match matcher::get_osu_match_id(&match_url_2) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";
            command.error(content).await?;

            return Ok(());
        }
    };

    if match_id1 == match_id2 {
        let content = "Trying to compare a match with itself huh";
        command.error(content).await?;

        return Ok(());
    }

    let match_fut1 = Context::osu().osu_match(match_id1);
    let match_fut2 = Context::osu().osu_match(match_id2);

    let output = output.unwrap_or_default();
    let comparison = comparison.unwrap_or_default();

    let pagination = match tokio::try_join!(match_fut1, match_fut2) {
        Ok((mut match1, mut match2)) => {
            let previous_fut_1 = retrieve_previous(&mut match1, Context::osu());
            let previous_fut_2 = retrieve_previous(&mut match2, Context::osu());

            if let Err(err) = tokio::try_join!(previous_fut_1, previous_fut_2) {
                let _ = command.error(OSU_API_ISSUE).await;
                let err = Report::new(err)
                    .wrap_err("Failed to get history of at least one of the matches");

                return Err(err);
            }

            let owner = command.user_id()?;

            MatchComparePagination::new(&mut match1, &mut match2, comparison, owner)
        }
        Err(OsuError::NotFound) => {
            let content = "At least one of the two given matches was not found";
            command.error(content).await?;

            return Ok(());
        }
        Err(OsuError::Response { status, .. }) if status == 401 => {
            let content =
                "I can't access at least one of the two matches because it was set as private";
            command.error(content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get at least one of the matches");

            return Err(report);
        }
    };

    match output {
        MatchCompareOutput::Full => {
            let mut embeds = pagination.into_embeds().into_iter();

            if let Some(embed) = embeds.next() {
                let builder = MessageBuilder::new().embed(embed);
                command.update(builder).await?;

                let mut interval = interval(Duration::from_secs(1));
                interval.tick().await;

                for embed in embeds {
                    interval.tick().await;

                    command
                        .channel_id
                        .create_message(embed.into(), command.permissions)
                        .await?;
                }
            }
        }
        MatchCompareOutput::Paginated => {
            ActiveMessages::builder(pagination)
                .start_by_update(true)
                .begin(&mut command)
                .await?;
        }
    }

    Ok(())
}
