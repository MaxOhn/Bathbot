use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    fmt::Write,
    iter, mem,
};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{
    constants::GENERAL_ISSUE, matcher, numbers::WithComma, EmbedBuilder, IntHasher, MessageBuilder,
    TourneyBadges,
};
use eyre::{Report, Result};
use rkyv::rancor::{Panic, ResultExt};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found};
use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, CachedUserExt, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "bws",
    desc = "Show the badge weighted seeding for an osu!standard player",
    help = "To combat those pesky derank players ruining everyone's tourneys, \
    many tournaments use a \"Badge Weighted Seeding\" system to adjust a player's rank based \
    on the amount of badges they own.\n\
    Instead of considering a player's global rank at face value, tourneys calculate \
    the player's bws value and use that to determine if they are allowed to \
    participate based on the rank restrictions.\n\
    There are various formulas around but this command uses `rank^(0.9937^(badges^2))`."
)]
pub struct Bws<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        desc = "Specify a target rank to reach",
        help = "If specified, it will calculate how the bws value would evolve towards the given rank."
    )]
    rank: Option<u32>,
    #[command(
        min_value = 0,
        desc = "Specify an amount of badges to reach",
        help = "Calculate how the bws value evolves towards the given amount of badges.\n\
        If none is specified, it defaults to the current amount + 2."
    )]
    badges: Option<usize>,
    #[command(
        min_value = 0,
        max_value = 3000,
        desc = "Filter out badges before a certain year"
    )]
    year: Option<i32>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

impl<'m> Bws<'m> {
    fn args(args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut rank = None;
        let mut badges = None;

        for arg in args.take(3) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "rank" | "r" => match value.parse::<u32>() {
                        Ok(num) => rank = Some(num.max(1)),
                        Err(_) => {
                            let content = "Failed to parse `rank`. Must be a positive integer.";

                            return Err(content.into());
                        }
                    },
                    "badges" | "badge" | "b" => match value.parse() {
                        Ok(num) => badges = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `badges`. Must be a positive integer.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\nAvailable options are: `rank` or `badges`."
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Ok(Self {
            name,
            rank,
            badges,
            year: None,
            discord,
        })
    }
}

async fn slash_bws(mut command: InteractionCommand) -> Result<()> {
    let args = Bws::from_interaction(command.input_data())?;

    bws((&mut command).into(), args).await
}

#[command]
#[desc("Show the badge weighted seeding for a player")]
#[help(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify `rank=integer` \
    to show how the bws value progresses towards that rank.\n\
    Similarly, you can specify `badges=integer` to show how the value \
    progresses towards that badge amount."
)]
#[usage("[username] [rank=integer] [badges=integer]")]
#[examples("badewanne3", "badewanne3 rank=1234 badges=10", "badewanne3 badges=3")]
#[group(Osu)]
async fn prefix_bws(msg: &Message, args: Args<'_>) -> Result<()> {
    match Bws::args(args) {
        Ok(args) => bws(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

const MIN_BADGES_OFFSET: usize = 2;

async fn bws(orig: CommandOrigin<'_>, args: Bws<'_>) -> Result<()> {
    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match Context::user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&orig).await,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let Bws {
        rank, badges, year, ..
    } = args;

    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let badges_iter = user
        .badges
        .iter()
        .filter(|badge| {
            let Some(year) = year else { return true };
            let awarded_at = badge.awarded_at.try_deserialize::<Panic>().always_ok();

            awarded_at.year() >= year
        })
        .map(|badge| &badge.description);

    let badges_curr = TourneyBadges::count(badges_iter);

    let (badges_min, badges_max) = match badges {
        Some(num) => {
            let mut min = num;
            let mut max = badges_curr;

            if min > max {
                mem::swap(&mut min, &mut max);
            }

            max += MIN_BADGES_OFFSET.saturating_sub(max - min);

            (min, max)
        }
        None => (badges_curr, badges_curr + MIN_BADGES_OFFSET),
    };

    let embed = bws_embed(&user, badges_curr, badges_min, badges_max, rank, year);
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

fn bws_embed(
    user: &CachedUser,
    badges_curr: usize,
    badges_min: usize,
    badges_max: usize,
    rank: Option<u32>,
    year: Option<i32>,
) -> EmbedBuilder {
    let global_rank = user
        .statistics
        .as_ref()
        .expect("missing stats")
        .global_rank
        .to_native();

    let dist_badges = badges_max - badges_min;
    let step_dist = 2;

    let badges: Vec<_> = (badges_min..badges_max)
        .step_by(dist_badges / step_dist)
        .take(step_dist)
        .chain(iter::once(badges_max))
        .map(|count| BadgeEntry {
            count,
            len: WithComma::new(count).to_string().len(),
        })
        .collect();

    let yellow = "\u{001b}[1;33m";
    let reset = "\u{001b}[0m";

    let description = match rank {
        Some(rank_arg) => {
            let mut min = rank_arg;
            let mut max = global_rank;

            if min > max {
                mem::swap(&mut min, &mut max);
            }

            let rank_len = max.to_string().len().max(6) + 1;
            let dist_rank = (max - min) as usize;
            let step_rank = 3;

            let bwss: BTreeMap<_, _> = {
                let mut values = HashSet::with_hasher(IntHasher);

                (min..max)
                    .step_by((dist_rank / step_rank).max(1))
                    .take(step_rank)
                    .chain(iter::once(max))
                    .filter(|&n| values.insert(n))
                    .map(|rank| {
                        let bwss: Vec<_> = badges
                            .iter()
                            .map(|entry| WithComma::new(bws_value(rank, entry.count)).to_string())
                            .collect();

                        (rank, bwss)
                    })
                    .collect()
            };

            // Calculate the widths for each column
            let max: Vec<_> = (0..=2)
                .map(|n| {
                    bwss.values()
                        .map(|bwss| bwss.get(n).unwrap().len())
                        .fold(0, |max, next| max.max(next))
                        .max(2)
                        .max(badges[n].len)
                })
                .collect();

            let mut content = String::with_capacity(256);
            content.push_str("```ansi\n");

            let _ = writeln!(
                content,
                " {:>rank_len$} | {:^len1$} | {:^len2$} | {:^len3$}",
                "Badges>",
                badges[0].count,
                badges[1].count,
                badges[2].count,
                len1 = max[0],
                len2 = max[1],
                len3 = max[2],
            );

            let _ = writeln!(
                content,
                "-{0:->rank_len$}-+-{0:-^len1$}-+-{0:-^len2$}-+-{0:-^len3$}-",
                '-',
                len1 = max[0],
                len2 = max[1],
                len3 = max[2],
            );

            for (rank, bwss) in bwss {
                let _ = writeln!(
                content,
                " {:>rank_len$} | {ansi_left}{:^len1$}{reset} | {:^len2$} | {ansi_right}{:^len3$}{reset}",
                format!("#{rank}"),
                bwss[0],
                bwss[1],
                bwss[2],
                len1 = max[0],
                len2 = max[1],
                len3 = max[2],
                ansi_left = if rank == global_rank && badges_curr == badges[0].count { yellow } else { reset },
                ansi_right = if rank == global_rank && badges_curr == badges[2].count { yellow } else { reset },
            );
            }

            content.push_str("```");

            content
        }
        None => {
            let bws1 = WithComma::new(bws_value(global_rank, badges[0].count)).to_string();
            let bws2 = WithComma::new(bws_value(global_rank, badges[1].count)).to_string();
            let bws3 = WithComma::new(bws_value(global_rank, badges[2].count)).to_string();
            let len1 = bws1.len().max(2).max(badges[0].len);
            let len2 = bws2.len().max(2).max(badges[1].len);
            let len3 = bws3.len().max(2).max(badges[2].len);
            let mut content = String::with_capacity(128);
            content.push_str("```ansi\n");

            let _ = writeln!(
                content,
                "Badges | {:^len1$} | {:^len2$} | {:^len3$}",
                badges[0].count, badges[1].count, badges[2].count,
            );

            let _ = writeln!(
                content,
                "-------+-{0:-^len1$}-+-{0:-^len2$}-+-{0:-^len3$}-",
                '-'
            );

            let _ = writeln!(
            content,
            "   BWS | {ansi_left}{bws1:^len1$}{reset} | {bws2:^len2$} | {ansi_right}{bws3:^len3$}{reset}\n```",
            ansi_left = if badges_curr == badges[0].count { yellow } else { reset },
            ansi_right = if badges_curr == badges[2].count { yellow } else { reset },
        );

            content
        }
    };

    let title = format!(
        "Current BWS for {badges_curr} badge{}: {}",
        if badges_curr == 1 { "" } else { "s" },
        WithComma::new(bws_value(global_rank, badges_curr))
    );

    let mut embed = EmbedBuilder::new();

    if let Some(year) = year {
        embed = embed.footer(format!("Badges from the year {year} onward"));
    }

    embed
        .author(user.author_builder(false))
        .description(description)
        .thumbnail(user.avatar_url.as_ref().to_owned())
        .title(title)
}

struct BadgeEntry {
    count: usize,
    /// Length of `count` when stringified
    len: usize,
}

fn bws_value(rank: u32, badges: usize) -> u64 {
    let rank = rank as f64;
    let badges = badges as i32;

    rank.powf(0.9937_f64.powi(badges * badges)).round() as u64
}
