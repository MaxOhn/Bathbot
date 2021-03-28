use crate::{
    commands::osu::{CompareResult, MinMaxAvgBasic},
    embeds::attachment,
    util::{
        datetime::sec_to_minsec,
        numbers::{with_comma_float, with_comma_uint},
    },
};

use chrono::{DateTime, Utc};
use rosu_v2::prelude::{GameMode, User, UserStatistics};
use std::{
    cmp::Reverse,
    fmt::{Display, Write},
};

pub struct ProfileCompareEmbed {
    description: String,
    image: String,
}

impl ProfileCompareEmbed {
    pub fn new(
        mode: GameMode,
        user1: User,
        user2: User,
        result1: CompareResult,
        result2: CompareResult,
    ) -> Self {
        let stats1 = user1.statistics.as_ref().unwrap();
        let stats2 = user2.statistics.as_ref().unwrap();

        let left = CompareStrings::new(stats1, user1.join_date, &result1);
        let max_left = left.max().max(user1.username.chars().count());
        let right = CompareStrings::new(stats2, user2.join_date, &result2);
        let max_right = right.max().max(user2.username.chars().count());
        let mut d = String::with_capacity(512);
        d.push_str("```\n");

        let _ = writeln!(
            d,
            "{:>max_left$}  | {:^12} |  {:<max_right$}",
            user1.username,
            match mode {
                GameMode::STD => "osu!",
                GameMode::MNA => "Mania",
                GameMode::TKO => "Taiko",
                GameMode::CTB => "CtB",
            },
            user2.username,
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

        write_line(
            &mut d,
            "Rank",
            left.rank,
            right.rank,
            Reverse(stats1.global_rank),
            Reverse(stats2.global_rank),
            max_left,
            max_right,
        );

        write_line(
            &mut d, "PP", left.pp, right.pp, stats1.pp, stats2.pp, max_left, max_right,
        );

        write_line(
            &mut d,
            "Accuracy",
            left.accuracy,
            right.accuracy,
            stats1.accuracy,
            stats2.accuracy,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Level",
            left.level,
            right.level,
            stats1.level.current,
            stats2.level.current,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Playtime",
            left.play_time,
            right.play_time,
            stats1.playtime,
            stats2.playtime,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Playcount",
            left.play_count,
            right.play_count,
            stats1.playcount,
            stats2.playcount,
            max_left,
            max_right,
        );

        let left_peak = user1
            .monthly_playcounts
            .unwrap()
            .iter()
            .map(|date_count| date_count.count as u64)
            .max()
            .unwrap_or(0);

        let right_peak = user2
            .monthly_playcounts
            .unwrap()
            .iter()
            .map(|date_count| date_count.count as u64)
            .max()
            .unwrap_or(0);

        write_line(
            &mut d,
            "PC peak",
            with_comma_uint(left_peak).to_string(),
            with_comma_uint(right_peak).to_string(),
            left_peak,
            right_peak,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Ranked score",
            left.ranked_score,
            right.ranked_score,
            stats1.ranked_score,
            stats2.ranked_score,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Total score",
            left.total_score,
            right.total_score,
            stats1.total_score,
            stats2.total_score,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Total hits",
            left.total_hits,
            right.total_hits,
            stats1.total_hits,
            stats2.total_hits,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "SS count",
            left.count_ss,
            right.count_ss,
            stats1.grade_counts.ss + stats1.grade_counts.ssh,
            stats2.grade_counts.ss + stats2.grade_counts.ssh,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "S count",
            left.count_s,
            right.count_s,
            stats1.grade_counts.s + stats1.grade_counts.sh,
            stats2.grade_counts.s + stats2.grade_counts.sh,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "A count",
            left.count_a,
            right.count_a,
            stats1.grade_counts.a + stats1.grade_counts.a,
            stats2.grade_counts.a + stats2.grade_counts.a,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Max Combo",
            with_comma_uint(stats1.max_combo).to_string(),
            with_comma_uint(stats2.max_combo).to_string(),
            stats1.max_combo,
            stats2.max_combo,
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Bonus PP",
            left.bonus_pp,
            right.bonus_pp,
            left.bonus_pp_num,
            right.bonus_pp_num,
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
            "Join date",
            user1.join_date.format("%F"),
            user2.join_date.format("%F"),
            Reverse(user1.join_date),
            Reverse(user2.join_date),
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

        let medal1 = user1.medals.unwrap().len();
        let medal2 = user2.medals.unwrap().len();

        write_line(
            &mut d, "Medals", medal1, medal2, medal1, medal2, max_left, max_right,
        );

        let badges1 = user1.badges.unwrap().len();
        let badges2 = user2.badges.unwrap().len();

        write_line(
            &mut d, "Badges", badges1, badges2, badges1, badges2, max_left, max_right,
        );

        write_line(
            &mut d,
            "Followers",
            with_comma_uint(user1.follower_count.unwrap_or(0)).to_string(),
            with_comma_uint(user2.follower_count.unwrap_or(0)).to_string(),
            user1.follower_count.unwrap(),
            user2.follower_count.unwrap(),
            max_left,
            max_right,
        );

        write_line(
            &mut d,
            "Replays seen",
            with_comma_uint(stats1.replays_watched).to_string(),
            with_comma_uint(stats2.replays_watched).to_string(),
            stats1.replays_watched,
            stats2.replays_watched,
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

impl_into_builder!(ProfileCompareEmbed { description, image });

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
            winner_left = if cmp_left > cmp_right { "<" } else { " " },
            winner_right = if cmp_left < cmp_right { ">" } else { " " },
        )
    );
}

struct CompareStrings {
    pp: String,
    rank: String,
    ranked_score: String,
    total_score: String,
    total_hits: String,
    play_count: String,
    play_time: String,
    level: String,
    bonus_pp: String,
    bonus_pp_num: f64,
    avg_map_len: String,
    accuracy: String,
    pp_per_month: String,
    pp_per_month_num: f32,
    count_ss: String,
    count_s: String,
    count_a: String,
    avg_pp: String,
    pp_spread: String,
}

impl CompareStrings {
    fn new(stats: &UserStatistics, join_date: DateTime<Utc>, result: &CompareResult) -> Self {
        let bonus_pow = 0.9994_f64.powi(
            (stats.grade_counts.ssh
                + stats.grade_counts.ss
                + stats.grade_counts.sh
                + stats.grade_counts.s
                + stats.grade_counts.a) as i32,
        );

        let bonus_pp_num = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;
        let days = (Utc::now() - join_date).num_days() as f32;
        let pp_per_month_num = 30.67 * stats.pp / days;

        Self {
            pp: with_comma_float(stats.pp).to_string() + "pp",
            rank: format!("#{}", with_comma_uint(stats.global_rank.unwrap_or(0))),
            ranked_score: with_comma_uint(stats.ranked_score).to_string(),
            total_score: with_comma_uint(stats.total_score).to_string(),
            total_hits: with_comma_uint(stats.total_hits).to_string(),
            play_count: with_comma_uint(stats.playcount).to_string(),
            play_time: with_comma_uint(stats.playtime / 3600).to_string() + "hrs",
            level: format!("{:.2}", stats.level.current),
            bonus_pp: format!("{:.2}pp", bonus_pp_num),
            bonus_pp_num,
            avg_map_len: sec_to_minsec(result.map_len.avg()),
            accuracy: format!("{:.2}%", stats.accuracy),
            pp_per_month: format!("{:.2}pp", pp_per_month_num),
            pp_per_month_num,
            count_ss: (stats.grade_counts.ssh + stats.grade_counts.ss).to_string(),
            count_s: (stats.grade_counts.sh + stats.grade_counts.s).to_string(),
            count_a: (stats.grade_counts.a).to_string(),
            avg_pp: format!("{:.2}pp", result.pp.avg()),
            pp_spread: format!("{:.2}pp", result.pp.max() - result.pp.min()),
        }
    }

    fn max(&self) -> usize {
        self.ranked_score
            .len()
            .max(self.total_score.len())
            .max(self.total_hits.len())
            .max(self.play_count.len())
            .max(self.play_time.len())
            .max(self.level.len())
            .max(self.bonus_pp.len())
            .max(self.rank.len())
            .max(self.pp.len())
            .max(self.avg_map_len.len())
            .max(self.accuracy.len())
            .max(self.pp_per_month.len())
            .max(self.count_ss.len())
            .max(self.count_s.len())
            .max(self.count_a.len())
            .max(self.avg_pp.len())
            .max(self.pp_spread.len())
            .max(10) // join date yyyy-mm-dd
    }
}
