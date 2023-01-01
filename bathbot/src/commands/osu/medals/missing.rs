use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use bathbot_macros::command;
use eyre::{Report, Result};
use hashbrown::HashSet;
use rkyv::{Deserialize, Infallible};
use rosu_v2::{prelude::OsuError, request::UserId};

use crate::{
    commands::osu::{require_link, user_not_found},
    core::commands::CommandOrigin,
    custom_client::{MedalGroup, OsekaiMedal, MEDAL_GROUPS},
    manager::redis::{osu::UserArgs, RedisData},
    pagination::MedalsMissingPagination,
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        hasher::IntHasher,
        matcher,
    },
    Context,
};

use super::MedalMissing;

#[command]
#[desc("Display a list of medals that a user is missing")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mm", "missingmedals")]
#[group(AllModes)]
async fn prefix_medalsmissing(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => MedalMissing {
                name: None,
                discord: Some(id),
            },
            None => MedalMissing {
                name: Some(Cow::Borrowed(arg)),
                discord: None,
            },
        },
        None => MedalMissing::default(),
    };

    missing(ctx, msg.into(), args).await
}

pub(super) async fn missing(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalMissing<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(owner).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;
    let user_fut = ctx.redis().osu_user(user_args);
    let medals_fut = ctx.redis().medals();

    let (user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        (_, Err(err)) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
        (Err(err), _) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let (user_medals_count, owned): (_, HashSet<_, IntHasher>) = match &user {
        RedisData::Original(user) => {
            let owned = user.medals.iter().map(|medal| medal.medal_id).collect();

            (user.medals.len(), owned)
        }
        RedisData::Archived(user) => {
            let owned = user.medals.iter().map(|medal| medal.medal_id).collect();

            (user.medals.len(), owned)
        }
    };

    let (medal_count, mut medals): (_, Vec<_>) = match all_medals {
        RedisData::Original(all_medals) => {
            let medal_count = (all_medals.len() - user_medals_count, all_medals.len());

            let medals = all_medals
                .into_iter()
                .filter(|medal| !owned.contains(&medal.medal_id))
                .map(MedalType::Medal)
                .collect();

            (medal_count, medals)
        }
        RedisData::Archived(all_medals) => {
            let medal_count = (all_medals.len() - user_medals_count, all_medals.len());

            let medals = all_medals
                .iter()
                .filter(|medal| !owned.contains(&medal.medal_id))
                .map(|entry| entry.deserialize(&mut Infallible).unwrap())
                .map(MedalType::Medal)
                .collect();

            (medal_count, medals)
        }
    };

    medals.extend(MEDAL_GROUPS.iter().copied().map(MedalType::Group));
    medals.sort_unstable();

    MedalsMissingPagination::builder(user, medals, medal_count)
        .start_by_update()
        .start(ctx, orig)
        .await
}

pub enum MedalType {
    Group(MedalGroup),
    Medal(OsekaiMedal),
}

impl MedalType {
    fn group(&self) -> MedalGroup {
        match self {
            Self::Group(g) => *g,
            Self::Medal(m) => m.grouping,
        }
    }
}

impl PartialEq for MedalType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MedalType::Group(a), MedalType::Group(b)) => a == b,
            (MedalType::Medal(a), MedalType::Medal(b)) => a.medal_id == b.medal_id,
            _ => false,
        }
    }
}

impl Eq for MedalType {}

impl PartialOrd for MedalType {
    fn partial_cmp(&self, other: &MedalType) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MedalType {
    fn cmp(&self, other: &MedalType) -> Ordering {
        self.group()
            .cmp(&other.group())
            .then_with(|| match (self, other) {
                (MedalType::Medal(a), MedalType::Medal(b)) => a.medal_id.cmp(&b.medal_id),
                (MedalType::Group(_), MedalType::Medal(_)) => Ordering::Less,
                (MedalType::Medal(_), MedalType::Group(_)) => Ordering::Greater,
                _ => unreachable!(),
            })
    }
}
