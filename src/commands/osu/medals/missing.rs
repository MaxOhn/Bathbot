use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use command_macros::command;
use eyre::{Report, Result};
use hashbrown::HashSet;
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{GameMode, OsuError};

use crate::{
    commands::osu::{get_user, require_link, UserArgs},
    core::commands::CommandOrigin,
    custom_client::{MedalGroup, OsekaiMedal, MEDAL_GROUPS},
    pagination::MedalsMissingPagination,
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
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

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(owner).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get username"));
            }
        },
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::Osu);
    let user_fut = get_user(&ctx, &user_args);
    let redis = ctx.redis();

    let (user, all_medals) = match tokio::join!(user_fut, redis.medals()) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{name}` was not found");

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

    let medals = user.medals.as_ref().unwrap();
    let archived_all_medals = all_medals.get();
    let medal_count = (
        archived_all_medals.len() - medals.len(),
        archived_all_medals.len(),
    );
    let owned: HashSet<_> = medals.iter().map(|medal| medal.medal_id).collect();

    let mut medals: Vec<_> = archived_all_medals
        .iter()
        .filter(|medal| !owned.contains(&medal.medal_id))
        .map(|entry| entry.deserialize(&mut Infallible).unwrap())
        .map(MedalType::Medal)
        .collect();

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
