use std::sync::Arc;

use bathbot_model::OsuTrackerPpEntry;
use bathbot_util::constants::OSUTRACKER_ISSUE;
use eyre::Result;
use rkyv::{Deserialize, Infallible};

use super::PopularMapsPp;
use crate::{
    active::{impls::PopularMapsPagination, ActiveMessages},
    core::{Context, ContextExt},
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
            Self::Pp100 => 100,
            Self::Pp200 => 200,
            Self::Pp300 => 300,
            Self::Pp400 => 400,
            Self::Pp500 => 500,
            Self::Pp600 => 600,
            Self::Pp700 => 700,
            Self::Pp800 => 800,
            Self::Pp900 => 900,
            Self::Pp1000 => 1000,
            Self::Pp1100 => 1100,
            Self::Pp1200 => 1200,
            Self::Pp1300 => 1300,
        }
    }
}
