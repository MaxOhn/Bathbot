use std::{borrow::Cow, cmp::Ordering};

use bathbot_macros::command;
use bathbot_model::{MedalGroup, OsekaiMedal, MEDAL_GROUPS};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
    matcher, IntHasher,
};
use eyre::{Report, Result};
use hashbrown::HashSet;
use rkyv::{
    rancor::{Panic, ResultExt},
    Deserialize,
};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};

use super::{MedalMissing, MedalMissingOrder};
use crate::{
    active::{impls::MedalsMissingPagination, ActiveMessages},
    commands::osu::{require_link, user_not_found},
    core::commands::CommandOrigin,
    manager::redis::osu::{UserArgs, UserArgsError},
    Context,
};

#[command]
#[desc("Display a list of medals that a user is missing")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mm", "missingmedals")]
#[group(AllModes)]
async fn prefix_medalsmissing(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => MedalMissing {
                name: None,
                sort: None,
                discord: Some(id),
            },
            None => MedalMissing {
                name: Some(Cow::Borrowed(arg)),
                sort: None,
                discord: None,
            },
        },
        None => MedalMissing::default(),
    };

    missing(msg.into(), args).await
}

pub(super) async fn missing(orig: CommandOrigin<'_>, args: MedalMissing<'_>) -> Result<()> {
    let owner = orig.user_id()?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match Context::user_config().osu_id(owner).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&orig).await,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;
    let user_fut = Context::redis().osu_user(user_args);
    let medals_fut = Context::redis().medals();

    let (user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(UserArgsError::Osu(OsuError::NotFound)), _) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        (_, Err(err)) => {
            let _ = orig.error(OSEKAI_ISSUE).await;

            return Err(err.wrap_err("Failed to get cached medals"));
        }
        (Err(err), _) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("Failed to get user");

            return Err(report);
        }
    };

    let user_medals_count = user.medals.len();
    let owned: Vec<_> = user
        .medals
        .iter()
        .map(|medal| medal.medal_id.to_native())
        .collect();

    let medal_count = (all_medals.len() - user_medals_count, all_medals.len());

    let mut medals: Vec<_> = all_medals
        .iter()
        .filter(|medal| !owned.contains(&medal.medal_id.to_native()))
        .map(|entry| rkyv::api::deserialize_using::<_, _, Panic>(entry, &mut ()).always_ok())
        .map(MedalType::Medal)
        .collect();

    medals.extend(MEDAL_GROUPS.iter().copied().map(MedalType::Group));

    let sort = args.sort.unwrap_or_default();

    match sort {
        MedalMissingOrder::Alphabet => medals.sort_unstable_by(|a, b| {
            a.group().cmp(&b.group()).then_with(|| match (a, b) {
                (MedalType::Group(_), MedalType::Medal(_)) => Ordering::Less,
                (MedalType::Medal(_), MedalType::Group(_)) => Ordering::Greater,
                (MedalType::Medal(a), MedalType::Medal(b)) => a.name.cmp(&b.name),
                (MedalType::Group(_), MedalType::Group(_)) => unreachable!(),
            })
        }),
        MedalMissingOrder::MedalId => medals.sort_unstable_by(|a, b| {
            a.group().cmp(&b.group()).then_with(|| match (a, b) {
                (MedalType::Group(_), MedalType::Medal(_)) => Ordering::Less,
                (MedalType::Medal(_), MedalType::Group(_)) => Ordering::Greater,
                (MedalType::Medal(a), MedalType::Medal(b)) => a.medal_id.cmp(&b.medal_id),
                (MedalType::Group(_), MedalType::Group(_)) => unreachable!(),
            })
        }),
        MedalMissingOrder::Rarity => medals.sort_unstable_by(|a, b| {
            a.group().cmp(&b.group()).then_with(|| match (a, b) {
                (MedalType::Group(_), MedalType::Medal(_)) => Ordering::Less,
                (MedalType::Medal(_), MedalType::Group(_)) => Ordering::Greater,
                (MedalType::Medal(a), MedalType::Medal(b)) => b.rarity.total_cmp(&a.rarity),
                (MedalType::Group(_), MedalType::Group(_)) => unreachable!(),
            })
        }),
    }

    let pagination = MedalsMissingPagination::builder()
        .user(user)
        .medals(medals.into_boxed_slice())
        .medal_count(medal_count)
        .sort(sort)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
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
