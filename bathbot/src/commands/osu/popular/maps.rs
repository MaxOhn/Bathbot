use std::sync::Arc;

use eyre::Result;
use rkyv::{Deserialize, Infallible};

use crate::{
    core::Context,
    custom_client::OsuTrackerPpEntry,
    manager::redis::RedisData,
    pagination::OsuTrackerMapsPagination,
    util::{constants::OSUTRACKER_ISSUE, interaction::InteractionCommand, InteractionCommandExt},
};

use super::PopularMapsPp;

pub(super) async fn maps(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: PopularMapsPp,
) -> Result<()> {
    let pp = args.pp();

    let entries: Vec<OsuTrackerPpEntry> = match ctx.redis().osutracker_pp_group(pp).await {
        Ok(RedisData::Original(group)) => group.list,
        Ok(RedisData::Archived(group)) => group.list.deserialize(&mut Infallible).unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osutracker pp groups"));
        }
    };

    OsuTrackerMapsPagination::builder(pp, entries)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
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
        }
    }
}
