use std::sync::Arc;

use eyre::Report;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::Context,
    embeds::EmbedData,
    embeds::OsuTrackerMapsEmbed,
    pagination::{OsuTrackerMapsPagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSUTRACKER_ISSUE},
        numbers, Authored, ApplicationCommandExt,
    },
    BotResult,
};

use super::PopularMapsPp;

pub(super) async fn maps(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    args: PopularMapsPp,
) -> BotResult<()> {
    let pp = args.pp();

    let entries = match ctx.client().get_osutracker_pp_groups().await {
        Ok(groups) => match groups.into_iter().find(|group| group.number == pp) {
            Some(group) => group.list,
            None => {
                error!("received no osutracker pp group with number={pp}");
                command.error(&ctx, GENERAL_ISSUE).await?;

                return Ok(());
            }
        },
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    let pages = numbers::div_euclid(10, entries.len());
    let initial = &entries[..entries.len().min(10)];

    let embed = OsuTrackerMapsEmbed::new(pp, initial, (1, pages)).into_builder();
    let builder = MessageBuilder::new().embed(embed.build());

    let response_raw = command.update(&ctx, &builder).await?;

    if entries.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerMapsPagination::new(response, pp, entries);
    let owner = command.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

impl PopularMapsPp {
    fn pp(self) -> u32 {
        match self {
            PopularMapsPp::Pp100 => 100,
            PopularMapsPp::Pp200 => 200,
            PopularMapsPp::Pp300 => 300,
            PopularMapsPp::Pp400 => 400,
            PopularMapsPp::Pp500 => 500,
            PopularMapsPp::Pp600 => 600,
            PopularMapsPp::Pp700 => 700,
            PopularMapsPp::Pp800 => 800,
            PopularMapsPp::Pp900 => 900,
            PopularMapsPp::Pp1000 => 1000,
            PopularMapsPp::Pp1100 => 1100,
            PopularMapsPp::Pp1200 => 1200,
        }
    }
}
