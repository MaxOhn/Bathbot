use std::sync::Arc;

use eyre::Report;

use crate::{
    core::{commands::CommandData, Context},
    embeds::EmbedData,
    embeds::OsuTrackerMapsEmbed,
    pagination::{OsuTrackerMapsPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSUTRACKER_ISSUE},
        numbers, MessageExt,
    },
    BotResult,
};

pub(super) async fn maps_(ctx: Arc<Context>, data: CommandData<'_>, pp: u32) -> BotResult<()> {
    let entries = match ctx.clients.custom.get_osutracker_pp_groups().await {
        Ok(groups) => match groups.into_iter().find(|group| group.number == pp) {
            Some(group) => group.list,
            None => {
                error!("received no osutracker pp group with number={pp}");

                return data.error(&ctx, GENERAL_ISSUE).await;
            }
        },
        Err(err) => {
            let _ = data.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    let pages = numbers::div_euclid(10, entries.len());
    let initial = &entries[..entries.len().min(10)];

    let embed = OsuTrackerMapsEmbed::new(pp, initial, (1, pages))
        .into_builder()
        .build();

    let response_raw = data.create_message(&ctx, embed.into()).await?;

    if entries.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerMapsPagination::new(response, pp, entries);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
