use std::{
    cmp::Reverse,
    fmt::{Display, Write},
};

use bathbot_macros::EmbedData;
use bathbot_model::{
    rkyv_util::time::DateTimeRkyv,
    rosu_v2::user::{StatsWrapper, User},
};
use bathbot_util::{
    datetime::{SecToMinSec, DATE_FORMAT},
    numbers::WithComma,
};
use rkyv::{with::DeserializeWith, Infallible};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;

use crate::{commands::osu::CompareResult, embeds::attachment, manager::redis::RedisData};

#[derive(EmbedData)]
pub struct ProfileCompareEmbed {
    description: String,
    image: String,
}

impl ProfileCompareEmbed {
    pub fn new(
        mode: GameMode,
        user1: &RedisData<User>,
        user2: &RedisData<User>,
        result1: CompareResult,
        result2: CompareResult,
    ) -> Self {
        let data1 = UserData::new(user1);
        let data2 = UserData::new(user2);

        let left = CompareStrings::new(&data1, &result1);
        let max_left = left.max().max(data1.username.chars().count());
        let right = CompareStrings::new(&data2, &result2);
        let max_right = right.max().max(data2.username.chars().count());
        let mut d = String::with_capacity(512);
        d.push_str("```\n");

        let _ = writeln!(
            d,
            "{:>max_left$}  | {:^12} |  {:<max_right$}",
            data1.username,
            match mode {
                GameMode::Osu => "osu!",
                GameMode::Mania => "Mania",
                GameMode::Taiko => "Taiko",
                GameMode::Catch => "CtB",
            },
            data2.username,
            max_left = max_left,
            max_right = max_right
        );

        let _ = writeln!(
            d,
            "{:->max_left$}--+-{:->12}-+--{:-<max_right$}",
            "-",
            "-",
            "-",
            max_left = max_left,
            max_right = max_right
        );

        let global_rank1 = data1.stats.global_rank();
        let global_rank2 = data2.stats.global_rank();

        write_line(
            &mut d,
            "Rank",
            left.rank,
            right.rank,
            Reverse(if global_rank1 == 0 {
                u32::MAX
            } else {
                global_rank1
            }),
            Reverse(if global_rank2 == 0 {
                u32::MAX
            } else {
                global_rank2
            }),
            max_left,
            max_right,
        );

        let left_peak = data1.highest_rank;
        let right_peak = data2.highest_rank;

        write_line(
            &mut d,
            "Peak rank",
            left_peak.map_or_else(|| "-".into(), |rank| format!("#{}", WithComma::new(rank))),
            right_peak.map_or_else(|| "-".into(), |rank| format!("#{}", WithComma::new(rank))),
            Reverse(left_peak.unwrap_or(u32::MAX)),
            Reverse(right_peak.unwrap_or(u32::MAX)),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "PP",
            left.pp,
            right.pp,
            data1.stats.pp(),
            data2.stats.pp(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Accuracy",
            left.accuracy,
            right.accuracy,
            data1.stats.accuracy(),
            data2.stats.accuracy(),
            max_left,
            max_right,
        );

        let level_left = data1.stats.level().float();
        let level_right = data2.stats.level().float();

        write_line(
            &mut d,
            "Level",
            level_left,
            level_right,
            level_left,
            level_right,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Playtime",
            left.play_time,
            right.play_time,
            data1.stats.playtime(),
            data2.stats.playtime(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Playcount",
            left.play_count,
            right.play_count,
            data1.stats.playcount(),
            data2.stats.playcount(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "PC peak",
            left.pc_peak,
            right.pc_peak,
            data1.monthly_playcounts_peak,
            data2.monthly_playcounts_peak,
            max_left,
            max_right,
        );

        if result1
            .score_rank_data
            .as_ref()
            .or(result2.score_rank_data.as_ref())
            .is_some()
        {
            write_line(
                &mut d,
                "Score rank",
                left.score_rank,
                right.score_rank,
                Reverse(
                    result1
                        .score_rank_data
                        .as_ref()
                        .map_or(u32::MAX, |user| user.rank),
                ),
                Reverse(
                    result2
                        .score_rank_data
                        .as_ref()
                        .map_or(u32::MAX, |user| user.rank),
                ),
                max_left,
                max_right,
            );

            write_line(
                &mut d,
                "Peak score rank",
                left.score_rank_peak,
                right.score_rank_peak,
                Reverse(result1.score_rank_data.as_ref().map_or(u32::MAX, |user| {
                    user.rank_highest
                        .as_ref()
                        .map_or(u32::MAX, |rank_highest| rank_highest.rank)
                })),
                Reverse(result2.score_rank_data.as_ref().map_or(u32::MAX, |user| {
                    user.rank_highest
                        .as_ref()
                        .map_or(u32::MAX, |rank_highest| rank_highest.rank)
                })),
                max_left,
                max_right,
            );
        }

        write_line(
            &mut d,
            "Ranked score",
            left.ranked_score,
            right.ranked_score,
            data1.stats.ranked_score(),
            data2.stats.ranked_score(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Total score",
            left.total_score,
            right.total_score,
            data1.stats.total_score(),
            data2.stats.total_score(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Total hits",
            left.total_hits,
            right.total_hits,
            data1.stats.total_hits(),
            data2.stats.total_hits(),
            max_left,
            max_right,
        );

        let grade_counts1 = data1.stats.grade_counts();
        let grade_counts2 = data2.stats.grade_counts();

        write_line(
            &mut d,
            "SS count",
            left.count_ss,
            right.count_ss,
            grade_counts1.ss + grade_counts1.ssh,
            grade_counts2.ss + grade_counts2.ssh,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "S count",
            left.count_s,
            right.count_s,
            grade_counts1.s + grade_counts1.sh,
            grade_counts2.s + grade_counts2.sh,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "A count",
            left.count_a,
            right.count_a,
            grade_counts1.a,
            grade_counts2.a,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Max Combo",
            left.max_combo,
            right.max_combo,
            data1.stats.max_combo(),
            data2.stats.max_combo(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Miss rate",
            left.miss_rate,
            right.miss_rate,
            left.miss_rate_num,
            right.miss_rate_num,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Miss percent",
            left.miss_percent,
            right.miss_percent,
            Reverse(left.miss_percent_num),
            Reverse(right.miss_percent_num),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Top1 PP",
            left.top1pp,
            right.top1pp,
            result1.top1pp,
            result2.top1pp,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Bonus PP",
            left.bonus_pp,
            right.bonus_pp,
            result1.bonus_pp,
            result2.bonus_pp,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "PP spread",
            left.pp_spread,
            right.pp_spread,
            result1.pp.max() - result1.pp.min(),
            result2.pp.max() - result2.pp.min(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Avg PP",
            left.avg_pp,
            right.avg_pp,
            result1.pp.avg(),
            result2.pp.avg(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "PP per month",
            left.pp_per_month,
            right.pp_per_month,
            left.pp_per_month_num,
            right.pp_per_month_num,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "PC per month",
            left.pc_per_month,
            right.pc_per_month,
            left.pc_per_month_num,
            right.pc_per_month_num,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Join date",
            data1.join_date.format(DATE_FORMAT).unwrap(),
            data2.join_date.format(DATE_FORMAT).unwrap(),
            Reverse(data1.join_date),
            Reverse(data2.join_date),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Avg map len",
            left.avg_map_len,
            right.avg_map_len,
            result1.map_len.avg(),
            result2.map_len.avg(),
            max_left,
            max_right,
        );

        let medal1 = data1.medals;
        let medal2 = data2.medals;

        write_line(
            &mut d, "Medals", medal1, medal2, medal1, medal2, max_left, max_right,
        );

        let badges1 = data1.badges;
        let badges2 = data2.badges;

        write_line(
            &mut d, "Badges", badges1, badges2, badges1, badges2, max_left, max_right,
        );

        write_line(
            &mut d,
            "Followers",
            left.followers,
            right.followers,
            data1.follower_count,
            data2.follower_count,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Replays seen",
            left.replays_seen,
            right.replays_seen,
            data1.stats.replays_watched(),
            data2.stats.replays_watched(),
            max_left,
            max_right,
        );

        d.push_str("```");

        Self {
            description: d,
            image: attachment("avatar_fuse.png"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_line<T: PartialOrd, V: Display>(
    content: &mut String,
    title: &str,
    left: V,
    right: V,
    cmp_left: T,
    cmp_right: T,
    max_left: usize,
    max_right: usize,
) {
    let _ = writeln!(
        content,
        "{:>max_left$} {winner_indicator} {:<max_right$}",
        left,
        right,
        max_left = max_left,
        max_right = max_right,
        winner_indicator = format_args!(
            "{winner_left}| {:^12} |{winner_right}",
            title,
            winner_left = if cmp_left > cmp_right { '<' } else { ' ' },
            winner_right = if cmp_left < cmp_right { '>' } else { ' ' },
        )
    );
}

struct CompareStrings {
    pp: Box<str>,
    rank: Box<str>,
    score_rank: Box<str>,
    score_rank_peak: Box<str>,
    ranked_score: Box<str>,
    total_score: Box<str>,
    total_hits: Box<str>,
    play_count: Box<str>,
    play_time: Box<str>,
    pc_peak: Box<str>,
    level: Box<str>,
    top1pp: Box<str>,
    bonus_pp: Box<str>,
    avg_map_len: Box<str>,
    accuracy: Box<str>,
    pp_per_month: Box<str>,
    pp_per_month_num: f32,
    pc_per_month: Box<str>,
    pc_per_month_num: f32,
    count_ss: Box<str>,
    count_s: Box<str>,
    count_a: Box<str>,
    avg_pp: Box<str>,
    pp_spread: Box<str>,
    max_combo: Box<str>,
    miss_rate: Box<str>,
    miss_rate_num: u32,
    miss_percent: Box<str>,
    miss_percent_num: f32,
    followers: Box<str>,
    replays_seen: Box<str>,
}

impl CompareStrings {
    fn new(data: &UserData<'_>, result: &CompareResult) -> Self {
        let UserData { stats, .. } = data;

        let days = (OffsetDateTime::now_utc() - data.join_date).whole_days() as f32;
        let pp_per_month_num = 30.67 * stats.pp() / days;
        let pc_per_month_num = 30.67 * stats.playcount() as f32 / days;

        let miss_rate = MissRate {
            misses: result.misses,
            hits: result.hits,
        };

        let (miss_percent, miss_percent_num) = miss_rate.percent();
        let (miss_rate, miss_rate_num) = miss_rate.rate();

        let grade_counts = stats.grade_counts();
        let global_rank = stats.global_rank();

        Self {
            pp: (WithComma::new(stats.pp()).to_string() + "pp").into_boxed_str(),
            rank: if global_rank == 0 {
                Box::from("-")
            } else {
                format!("#{}", WithComma::new(global_rank)).into_boxed_str()
            },
            score_rank: result.score_rank_data.as_ref().map_or_else(
                || Box::from("-"),
                |user| format!("#{}", WithComma::new(user.rank)).into_boxed_str(),
            ),
            score_rank_peak: result.score_rank_data.as_ref().map_or_else(
                || Box::from("-"),
                |user| {
                    user.rank_highest.as_ref().map_or_else(
                        || Box::from("-"),
                        |rank_highest| {
                            format!("#{}", WithComma::new(rank_highest.rank)).into_boxed_str()
                        },
                    )
                },
            ),
            ranked_score: WithComma::new(stats.ranked_score())
                .to_string()
                .into_boxed_str(),
            total_score: WithComma::new(stats.total_score())
                .to_string()
                .into_boxed_str(),
            total_hits: WithComma::new(stats.total_hits())
                .to_string()
                .into_boxed_str(),
            play_count: WithComma::new(stats.playcount())
                .to_string()
                .into_boxed_str(),
            play_time: (WithComma::new(stats.playtime() / 3600).to_string() + "hrs")
                .into_boxed_str(),
            level: format!("{:.2}", stats.level().float()).into_boxed_str(),
            top1pp: format!("{:.2}pp", result.top1pp).into_boxed_str(),
            bonus_pp: format!("{:.2}pp", result.bonus_pp).into_boxed_str(),
            avg_map_len: SecToMinSec::new(result.map_len.avg())
                .to_string()
                .into_boxed_str(),
            accuracy: format!("{:.2}%", stats.accuracy()).into_boxed_str(),
            pp_per_month: format!("{pp_per_month_num:.2}pp").into_boxed_str(),
            pp_per_month_num,
            pc_per_month: format!("{pc_per_month_num:.2}").into_boxed_str(),
            pc_per_month_num,
            count_ss: (grade_counts.ssh + grade_counts.ss)
                .to_string()
                .into_boxed_str(),
            count_s: (grade_counts.sh + grade_counts.s)
                .to_string()
                .into_boxed_str(),
            count_a: (grade_counts.a).to_string().into_boxed_str(),
            avg_pp: format!("{:.2}pp", result.pp.avg()).into_boxed_str(),
            pp_spread: format!("{:.2}pp", result.pp.max() - result.pp.min()).into_boxed_str(),
            pc_peak: WithComma::new(data.monthly_playcounts_peak)
                .to_string()
                .into_boxed_str(),
            max_combo: WithComma::new(stats.max_combo())
                .to_string()
                .into_boxed_str(),
            miss_rate,
            miss_rate_num,
            miss_percent,
            miss_percent_num,
            followers: WithComma::new(data.follower_count)
                .to_string()
                .into_boxed_str(),
            replays_seen: WithComma::new(stats.replays_watched())
                .to_string()
                .into_boxed_str(),
        }
    }

    fn max(&self) -> usize {
        let Self {
            pp,
            rank,
            score_rank,
            score_rank_peak,
            ranked_score: _,
            total_score,
            total_hits,
            play_count,
            play_time,
            pc_peak,
            level,
            top1pp,
            bonus_pp,
            avg_map_len,
            accuracy,
            pp_per_month,
            pp_per_month_num: _,
            pc_per_month,
            pc_per_month_num: _,
            count_ss,
            count_s,
            count_a,
            avg_pp,
            pp_spread,
            max_combo,
            miss_rate,
            miss_rate_num: _,
            miss_percent,
            miss_percent_num: _,
            followers,
            replays_seen,
        } = self;

        self.ranked_score
            .len()
            .max(score_rank.len())
            .max(score_rank_peak.len())
            .max(total_score.len())
            .max(total_hits.len())
            .max(play_count.len())
            .max(play_time.len())
            .max(level.len())
            .max(top1pp.len())
            .max(bonus_pp.len())
            .max(rank.len())
            .max(pp.len())
            .max(avg_map_len.len())
            .max(accuracy.len())
            .max(pp_per_month.len())
            .max(pc_per_month.len())
            .max(count_ss.len())
            .max(count_s.len())
            .max(count_a.len())
            .max(avg_pp.len())
            .max(pp_spread.len())
            .max(10) // join date yyyy-mm-dd
            .max(pc_peak.len())
            .max(max_combo.len())
            .max(miss_rate.len())
            .max(miss_percent.len())
            .max(followers.len())
            .max(replays_seen.len())
    }
}

struct UserData<'u> {
    stats: StatsWrapper<'u>,
    username: &'u str,
    join_date: OffsetDateTime,
    follower_count: u32,
    highest_rank: Option<u32>,
    monthly_playcounts_peak: i32,
    medals: usize,
    badges: usize,
}

impl<'u> UserData<'u> {
    fn new(user: &'u RedisData<User>) -> Self {
        match user {
            RedisData::Original(user) => Self {
                stats: StatsWrapper::Left(user.statistics.as_ref().expect("missing statistics")),
                username: user.username.as_str(),
                join_date: user.join_date,
                follower_count: user.follower_count,
                highest_rank: user.highest_rank.as_ref().map(|peak| peak.rank),
                monthly_playcounts_peak: user
                    .monthly_playcounts
                    .iter()
                    .map(|date_count| date_count.count)
                    .max()
                    .unwrap_or(0),
                medals: user.medals.len(),
                badges: user.badges.len(),
            },
            RedisData::Archive(user) => Self {
                stats: StatsWrapper::Right(user.statistics.as_ref().expect("missing statistics")),
                username: user.username.as_str(),
                join_date: DateTimeRkyv::deserialize_with(&user.join_date, &mut Infallible)
                    .unwrap(),
                follower_count: user.follower_count,
                highest_rank: user.highest_rank.as_ref().map(|peak| peak.rank),
                monthly_playcounts_peak: user
                    .monthly_playcounts
                    .iter()
                    .map(|date_count| date_count.count)
                    .max()
                    .unwrap_or(0),
                medals: user.medals.len(),
                badges: user.badges.len(),
            },
        }
    }
}

struct MissRate {
    misses: u32,
    hits: u32,
}

impl MissRate {
    fn rate(&self) -> (Box<str>, u32) {
        if self.misses == 0 {
            (
                format!("0m / {} hits", self.hits).into_boxed_str(),
                self.hits,
            )
        } else {
            let div = self.hits / self.misses;

            (format!("1m / {div} hits").into_boxed_str(), div)
        }
    }

    fn percent(&self) -> (Box<str>, f32) {
        if self.misses == 0 {
            (Box::from("0%"), 0.0)
        } else {
            let div = (100 * self.misses) as f32 / self.hits as f32;

            let s = if div < 0.001 {
                Box::from("<0.001%")
            } else {
                format!("{div:.3}%").into_boxed_str()
            };

            (s, div)
        }
    }
}
