use std::{cmp::Ordering, collections::HashMap};

use bathbot_macros::command;
use bathbot_model::{MEDAL_GROUPS, MedalGroup, OsekaiMedal};
use bathbot_util::{IntHasher, constants::GENERAL_ISSUE, matcher};
use eyre::{Report, Result};
use rkyv::rancor::{Panic, ResultExt};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use time::OffsetDateTime;
use twilight_model::guild::Permissions;

use super::{MedalList, MedalListOrder, icons_image::draw_icons_image};
use crate::{
    Context,
    active::{ActiveMessages, impls::MedalsListPagination},
    commands::osu::{medals::MEDAL_LIST_DESC, require_link, user_not_found},
    core::commands::{CommandOrigin, prefix::Args},
    manager::redis::osu::{UserArgs, UserArgsError},
};

impl<'m> MedalList<'m> {
    fn args(args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;

        for arg in args {
            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            name,
            discord,
            sort: None,
            group: None,
            reverse: None,
            grouped: None,
        }
    }
}

#[command]
#[desc(MEDAL_LIST_DESC)]
#[usage("[username]")]
#[example("brandwagen")]
#[aliases("ml", "medallist")]
#[group(AllModes)]
async fn prefix_medalslist(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);
    let args = MedalList::args(args);

    list(orig, args).await
}

pub(super) async fn list(orig: CommandOrigin<'_>, args: MedalList<'_>) -> Result<()> {
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

    let MedalList {
        sort,
        group,
        reverse,
        grouped,
        ..
    } = args;

    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;
    let user_fut = Context::redis().osu_user(user_args);
    let medals_fut = Context::redis().medals();
    let ranking_fut = Context::redis().osekai_rarity();

    let (user, osekai_medals, rarities) = match tokio::join!(user_fut, medals_fut, ranking_fut) {
        (Ok(user), Ok(medals), Ok(rarities)) => (user, medals, rarities),
        (Err(UserArgsError::Osu(OsuError::NotFound)), ..) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        (Err(err), ..) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("Failed to get user");

            return Err(report);
        }
        (_, Err(err), _) | (.., Err(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached rarity ranking"));
        }
    };

    let rarities: HashMap<_, _, IntHasher> = rarities
        .iter()
        .map(|entry| (entry.medal_id.to_native(), entry.frequency.to_native()))
        .collect();

    let acquired = (user.medals.len(), osekai_medals.len());

    let medals_iter = user.medals.iter().filter_map(|m| {
        match osekai_medals
            .iter()
            .position(|m_| m_.medal_id == m.medal_id)
        {
            Some(idx) => {
                let achieved = m.achieved_at.try_deserialize::<Panic>().always_ok();

                let entry = MedalEntryList {
                    medal: rkyv::api::deserialize_using::<_, _, Panic>(
                        &osekai_medals[idx],
                        &mut (),
                    )
                    .always_ok(),
                    achieved,
                    rarity: rarities
                        .get(&(m.medal_id.to_native() as u16))
                        .copied()
                        .unwrap_or(100.0),
                };

                Some(entry)
            }
            None => {
                warn!("Missing medal id {}", m.medal_id);

                None
            }
        }
    });

    let mut medals = Vec::with_capacity(acquired.0);
    medals.extend(medals_iter);

    if let Some(group) = group {
        medals.retain(|entry| entry.medal.grouping == group);
    }

    let sort = sort.unwrap_or_default();
    let grouped = grouped == Some(true);

    let order_str = match sort {
        MedalListOrder::Alphabet => "alphabet",
        MedalListOrder::Date => "date",
        MedalListOrder::MedalId => "medal id",
        MedalListOrder::Rarity => "rarity",
    };

    let mut entries: Vec<MedalListEntry> = medals.into_iter().map(MedalListEntry::Medal).collect();

    if grouped {
        for group in MEDAL_GROUPS {
            let present = entries.iter().any(
                |entry| matches!(entry, MedalListEntry::Medal(m) if m.medal.grouping == group),
            );

            if present {
                entries.push(MedalListEntry::Group(group));
            }
        }
    }

    let reverse = reverse == Some(true);

    let by_key = |a: &MedalEntryList, b: &MedalEntryList| {
        let ord = key_cmp(&sort, a, b);

        if reverse { ord.reverse() } else { ord }
    };

    entries.sort_unstable_by(|a, b| {
        if grouped {
            let group_a = match a {
                MedalListEntry::Group(group) => *group,
                MedalListEntry::Medal(medal) => medal.medal.grouping,
            };
            let group_b = match b {
                MedalListEntry::Group(group) => *group,
                MedalListEntry::Medal(medal) => medal.medal.grouping,
            };

            let group_cmp = if reverse {
                group_b.cmp(&group_a)
            } else {
                group_a.cmp(&group_b)
            };

            group_cmp.then_with(|| match (a, b) {
                (MedalListEntry::Group(_), MedalListEntry::Medal(_)) => Ordering::Less,
                (MedalListEntry::Medal(_), MedalListEntry::Group(_)) => Ordering::Greater,
                (MedalListEntry::Medal(a), MedalListEntry::Medal(b)) => by_key(a, b),
                (MedalListEntry::Group(_), MedalListEntry::Group(_)) => Ordering::Equal,
            })
        } else {
            match (a, b) {
                (MedalListEntry::Medal(a), MedalListEntry::Medal(b)) => by_key(a, b),
                _ => Ordering::Equal,
            }
        }
    });

    let reverse_str = if reverse { "reversed " } else { "" };

    let medal_ids: Vec<_> = entries
        .iter()
        .filter_map(|entry| match entry {
            MedalListEntry::Medal(medal) => Some(medal.medal.medal_id),
            MedalListEntry::Group(_) => None,
        })
        .collect();

    let image = match Context::redis().medal_icons(&medal_ids).await {
        Ok(mut icons) => {
            icons.sort_unstable_by(|(a, _), (b, _)| {
                let position_fn = |m: &MedalListEntry, id: u32| match m {
                    MedalListEntry::Medal(medal) => medal.medal.medal_id == id,
                    MedalListEntry::Group(_) => false,
                };

                let idx_a = entries.iter().position(|m| position_fn(m, *a));
                let idx_b = entries.iter().position(|m| position_fn(m, *b));

                idx_a.cmp(&idx_b)
            });

            match draw_icons_image(&icons) {
                Ok(image) => Some(image),
                Err(err) => {
                    warn!(?err, "Failed to draw image");

                    None
                }
            }
        }
        Err(err) => {
            warn!(?err);

            None
        }
    };

    let grouped_str = if grouped { " (grouped)" } else { "" };

    let name = user.username.as_str();

    let content = match group {
        None => {
            format!("All medals of `{name}` sorted by {reverse_str}{order_str}{grouped_str}:")
        }
        Some(group) => format!(
            "All `{group}` medals of `{name}` sorted by {reverse_str}{order_str}{grouped_str}:"
        ),
    };

    let pagination = MedalsListPagination::builder()
        .user(user)
        .acquired(acquired)
        .medals(entries.into_boxed_slice())
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(image.map(|image| (MedalsListPagination::IMAGE_NAME.to_owned(), image)))
        .begin(orig)
        .await
}

pub struct MedalEntryList {
    pub medal: OsekaiMedal,
    pub achieved: OffsetDateTime,
    pub rarity: f64,
}

pub enum MedalListEntry {
    Group(MedalGroup),
    Medal(MedalEntryList),
}

fn key_cmp(sort: &MedalListOrder, a: &MedalEntryList, b: &MedalEntryList) -> Ordering {
    match sort {
        MedalListOrder::Alphabet => a.medal.name.cmp(&b.medal.name),
        MedalListOrder::Date => b.achieved.cmp(&a.achieved),
        MedalListOrder::MedalId => a.medal.medal_id.cmp(&b.medal.medal_id),
        MedalListOrder::Rarity => a.rarity.partial_cmp(&b.rarity).unwrap_or(Ordering::Equal),
    }
}
