use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use bathbot_macros::SlashCommand;
use bathbot_model::{
    rkyv_util::time::DateRkyv,
    rosu_v2::user::{User, UserStatisticsRkyv},
};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use futures::{future, stream::FuturesUnordered, TryStreamExt};
use once_cell::sync::OnceCell;
use rkyv::{
    rancor::{Panic, ResultExt},
    Archived,
};
use rosu_v2::prelude::{CountryCode, GameMode, OsuError, UserHighestRank, Username};
use time::{OffsetDateTime, Time};
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::Context,
    embeds::{ClaimNameEmbed, EmbedData},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "claimname",
    desc = "Check how much longer to wait until a name is up for grabs",
    help = "If a player has not signed in for at least 6 months and has no plays,\
    their username may be claimed.\n\
    If that player does have any plays across all game modes, \
    a [non-linear function](https://www.desmos.com/calculator/b89siyv9j8) is used to calculate \
    how much extra time is added to those 6 months.\n\
    This is to prevent people from stealing the usernames of active or recently retired players."
)]
pub struct ClaimName {
    #[command(desc = "Specify a username")]
    name: String,
}

async fn slash_claimname(mut command: InteractionCommand) -> Result<()> {
    let ClaimName { name } = ClaimName::from_interaction(command.input_data())?;

    let content = if name.chars().count() > 15 {
        Some("Names can have at most 15 characters so your name won't be accepted".to_owned())
    } else if let Some(c) = name
        .chars()
        .find(|c| !matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '[' | ']' | '_' | ' '))
    {
        Some(format!(
            "`{c}` is an invalid character for usernames so `{name}` won't be accepted"
        ))
    } else if name.len() < 3 {
        Some(format!(
            "Names must be at least 3 characters long so `{name}` won't be accepted"
        ))
    } else if name.contains('_') && name.contains(' ') {
        Some(format!(
            "Names may contains underscores or spaces but not both \
            so `{name}` won't be accepted"
        ))
    } else if name.starts_with(' ') || name.ends_with(' ') {
        Some(format!(
            "Names can't start or end with spaces so `{name}` won't be accepted"
        ))
    } else {
        None
    };

    if let Some(content) = content {
        let builder = MessageBuilder::new().embed(content);
        command.update(builder).await?;

        return Ok(());
    }

    let user_id = match UserArgs::username(&name, GameMode::Osu).await {
        UserArgs::Args(args) => args.user_id,
        UserArgs::User { user, .. } => user.user_id.to_native(),
        UserArgs::Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = if ClaimNameValidator::is_valid(&name) {
                format!("User `{name}` was not found, the name should be available to claim")
            } else {
                format!("`{name}` does not seem to be taken but it likely won't be accepted")
            };

            let builder = MessageBuilder::new().embed(content);
            command.update(builder).await?;

            return Ok(());
        }
        UserArgs::Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let args = [
        GameMode::Osu,
        GameMode::Taiko,
        GameMode::Catch,
        GameMode::Mania,
    ]
    .map(|mode| UserArgs::user_id(user_id, mode));

    let user_fut = args
        .into_iter()
        .map(|args| Context::redis().osu_user(args))
        .collect::<FuturesUnordered<_>>()
        .try_fold(None, |user: Option<ClaimNameUser>, next| match user {
            Some(mut user) => {
                let next_stats = next
                    .statistics
                    .as_ref()
                    .map(|stats| {
                        rkyv::api::deserialize_using::<_, _, Panic>(stats, &mut ()).always_ok()
                    })
                    .expect("missing stats");

                match user.statistics {
                    Some(ref mut accum) => accum.playcount += next_stats.playcount,
                    None => user.statistics = Some(next_stats),
                }

                let next: &Archived<User> = &next;

                let next_highest_rank =
                    next.highest_rank
                        .as_ref()
                        .map(|highest_rank| UserHighestRank {
                            rank: highest_rank.rank.to_native(),
                            updated_at: highest_rank
                                .updated_at
                                .try_deserialize::<Panic>()
                                .always_ok(),
                        });

                let last_playcount = next
                    .monthly_playcounts
                    .iter()
                    .rev()
                    .find(|count| count.count > 0)
                    .map(|count| {
                        DateRkyv::try_deserialize(count.start_date)
                            .unwrap()
                            .with_time(Time::MIDNIGHT)
                            .assume_utc()
                    });

                let last_visit = next
                    .last_visit
                    .as_ref()
                    .map(|time| time.try_deserialize::<Panic>().always_ok());

                let next_last_visit = match (last_visit, last_playcount) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), _) | (_, Some(a)) => Some(a),
                    _ => None,
                };

                match (user.highest_rank.as_mut(), next_highest_rank) {
                    (Some(curr), Some(next)) if curr.rank > next.rank => *curr = next,
                    (None, next @ Some(_)) => user.highest_rank = next,
                    _ => {}
                }

                match (user.last_visit.as_mut(), next_last_visit) {
                    (Some(curr), Some(next)) if *curr < next => *curr = next,
                    (None, next @ Some(_)) => user.last_visit = next,
                    _ => {}
                }

                future::ready(Ok(Some(user)))
            }
            None => future::ready(Ok(Some(ClaimNameUser::from(next)))),
        });

    let user = match user_fut.await {
        Ok(user) => user.unwrap(),
        Err(err) => {
            let _ = command.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let embed = ClaimNameEmbed::new(&user, &name).build();
    let builder = MessageBuilder::new().embed(embed);
    command.update(builder).await?;

    Ok(())
}

pub struct ClaimNameUser {
    pub avatar_url: Box<str>,
    pub country_code: CountryCode,
    pub has_badges: bool,
    pub has_ranked_mapsets: bool,
    pub highest_rank: Option<UserHighestRank>,
    pub last_visit: Option<OffsetDateTime>,
    pub statistics: Option<UserStatisticsRkyv>,
    pub username: Username,
    pub user_id: u32,
}

impl From<User> for ClaimNameUser {
    #[inline]
    fn from(user: User) -> Self {
        Self {
            avatar_url: user.avatar_url,
            country_code: user.country_code,
            has_badges: !user.badges.is_empty(),
            has_ranked_mapsets: user.ranked_mapset_count > 0,
            highest_rank: user.highest_rank,
            last_visit: user.last_visit,
            statistics: user.statistics.as_ref().map(UserStatisticsRkyv::from),
            username: user.username,
            user_id: user.user_id,
        }
    }
}

impl From<CachedUser> for ClaimNameUser {
    fn from(user: CachedUser) -> Self {
        Self {
            avatar_url: user.avatar_url.as_ref().into(),
            country_code: user.country_code.as_str().into(),
            has_badges: !user.badges.is_empty(),
            has_ranked_mapsets: user.ranked_mapset_count > 0,
            highest_rank: user
                .highest_rank
                .as_ref()
                .map(|highest_rank| UserHighestRank {
                    rank: highest_rank.rank.to_native(),
                    updated_at: highest_rank
                        .updated_at
                        .try_deserialize::<Panic>()
                        .always_ok(),
                }),
            last_visit: user
                .last_visit
                .as_ref()
                .map(|time| time.try_deserialize::<Panic>().always_ok()),
            statistics: user.statistics.as_ref().map(|stats| {
                rkyv::api::deserialize_using::<_, _, Panic>(stats, &mut ()).always_ok()
            }),
            username: user.username.as_str().into(),
            user_id: user.user_id.to_native(),
        }
    }
}

pub struct ClaimNameValidator;

impl ClaimNameValidator {
    pub fn is_valid(prefix: &str) -> bool {
        !VALIDATOR
            .get_or_init(|| {
                let needles = [
                    "qfqqz",
                    "dppljf{",
                    "difbu",
                    "ojhhfs",
                    "mpmj",
                    "gvdl",
                    "ejmep",
                    "gbhhpu",
                    "dvou",
                    "tijhfupsb",
                    "qpso",
                    "cbodip",
                    "qfojt",
                    "wbhjob",
                    "qvttz",
                    "ejdl",
                    "dpdl",
                    "brvjmb",
                    "ijumfs",
                    "ibdl",
                    "tibwju",
                    "gsjfoepl",
                ]
                .into_iter()
                .map(String::from)
                .map(|mut needle| {
                    unsafe { needle.as_bytes_mut() }
                        .iter_mut()
                        .for_each(|byte| *byte -= 1);

                    needle
                });

                AhoCorasickBuilder::new()
                    .ascii_case_insensitive(true)
                    .build(needles)
                    .unwrap()
            })
            .is_match(prefix)
    }
}

static VALIDATOR: OnceCell<AhoCorasick> = OnceCell::new();
