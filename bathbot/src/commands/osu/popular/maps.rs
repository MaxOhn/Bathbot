use std::sync::Arc;

use bathbot_model::OsuTrackerPpEntry;
use bathbot_util::constants::OSUTRACKER_ISSUE;
use eyre::Result;
use rkyv::{Deserialize, Infallible};

use super::PopularMapsPp;
use crate::{
    active::{impls::PopularMapsPagination, ActiveMessages},
    core::Context,
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub(super) async fn maps(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: PopularMapsPp,
) -> Result<()> {
    let pp = args.pp();

    let entries: Vec<OsuTrackerPpEntry> = match ctx.redis().osutracker_pp_group(pp).await {
        Ok(RedisData::Original(group)) => group.list,
        Ok(RedisData::Archive(group)) => group.list.deserialize(&mut Infallible).unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osutracker pp groups"));
        }
    };

    let pagination = PopularMapsPagination::builder()
        .pp(pp)
        .entries(entries.into_boxed_slice())
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
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
