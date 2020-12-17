use crate::{
    commands::osu::{CompareResult, MinMaxAvgBasic},
    custom_client::OsuProfile,
    embeds::EmbedData,
    util::{
        datetime::sec_to_minsec,
        numbers::{with_comma, with_comma_u64},
    },
};

use chrono::Utc;
use rosu::model::{GameMode, User};
use std::{
    cmp::Reverse,
    fmt::{Display, Write},
};
use twilight_embed_builder::image_source::ImageSource;

pub struct ProfileCompareEmbed {
    description: Option<String>,
    image: Option<ImageSource>,
}

impl ProfileCompareEmbed {
    pub fn new(
        mode: GameMode,
        user1: User,
        user2: User,
        result1: CompareResult,
        result2: CompareResult,
        profile1: OsuProfile,
        profile2: OsuProfile,
    ) -> Self {
        let left = CompareStrings::new(&user1, &result1);
        let max_left = left.max().max(user1.username.chars().count());
        let right = CompareStrings::new(&user2, &result2);
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
            Reverse(user1.pp_rank),
            Reverse(user2.pp_rank),
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "PP",
            left.pp,
            right.pp,
            user1.pp_raw,
            user2.pp_raw,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Accuracy",
            left.accuracy,
            right.accuracy,
            user1.accuracy,
            user2.accuracy,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Level",
            left.level,
            right.level,
            user1.level,
            user2.level,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Playtime",
            left.play_time,
            right.play_time,
            user1.total_seconds_played,
            user2.total_seconds_played,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Playcount",
            left.play_count,
            right.play_count,
            user1.playcount,
            user2.playcount,
            max_left,
            max_right,
        );
        let left_peak = profile1
            .monthly_playcounts
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap_or(0);
        let right_peak = profile2
            .monthly_playcounts
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap_or(0);
        write_line(
            &mut d,
            "PC peak",
            with_comma_u64(left_peak as u64),
            with_comma_u64(right_peak as u64),
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
            user1.ranked_score,
            user2.ranked_score,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Total score",
            left.total_score,
            right.total_score,
            user1.total_score,
            user2.total_score,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Total hits",
            left.total_hits,
            right.total_hits,
            user1.total_hits(),
            user2.total_hits(),
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "SS count",
            left.count_ss,
            right.count_ss,
            user1.count_ss + user1.count_ssh,
            user2.count_ss + user2.count_ssh,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "S count",
            left.count_s,
            right.count_s,
            user1.count_s + user1.count_sh,
            user2.count_s + user2.count_sh,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "A count",
            left.count_a,
            right.count_a,
            user1.count_a,
            user2.count_a,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Max Combo",
            with_comma_u64(profile1.statistics.max_combo as u64),
            with_comma_u64(profile2.statistics.max_combo as u64),
            profile1.statistics.max_combo,
            profile2.statistics.max_combo,
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
        write_line(
            &mut d,
            "Medals",
            profile1.medals.len(),
            profile2.medals.len(),
            profile1.medals.len(),
            profile2.medals.len(),
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Badges",
            profile1.badges.len(),
            profile2.badges.len(),
            profile1.badges.len(),
            profile2.badges.len(),
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Followers",
            with_comma_u64(profile1.follower_count as u64),
            with_comma_u64(profile2.follower_count as u64),
            profile1.follower_count,
            profile2.follower_count,
            max_left,
            max_right,
        );
        write_line(
            &mut d,
            "Replays seen",
            with_comma_u64(profile1.statistics.replays_watched as u64),
            with_comma_u64(profile2.statistics.replays_watched as u64),
            profile1.statistics.replays_watched,
            profile2.statistics.replays_watched,
            max_left,
            max_right,
        );
        d.push_str("```");
        Self {
            description: Some(d),
            image: Some(ImageSource::attachment("avatar_fuse.png").unwrap()),
        }
    }
}

impl EmbedData for ProfileCompareEmbed {
    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }
    fn image_owned(&mut self) -> Option<ImageSource> {
        self.image.take()
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
    fn new(user: &User, result: &CompareResult) -> Self {
        let bonus_pow = 0.9994_f64.powi(
            (user.count_ssh + user.count_ss + user.count_sh + user.count_s + user.count_a) as i32,
        );
        let bonus_pp_num = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;
        let days = (Utc::now() - user.join_date).num_days() as f32;
        let pp_per_month_num = 30.67 * user.pp_raw / days;
        Self {
            pp: with_comma(user.pp_raw) + "pp",
            rank: format!("#{}", with_comma_u64(user.pp_rank as u64)),
            ranked_score: with_comma_u64(user.ranked_score),
            total_score: with_comma_u64(user.total_score),
            total_hits: with_comma_u64(user.total_hits()),
            play_count: with_comma_u64(user.playcount as u64),
            play_time: with_comma_u64(user.total_seconds_played as u64 / 3600) + "hrs",
            level: format!("{:.2}", user.level),
            bonus_pp: format!("{:.2}pp", bonus_pp_num),
            bonus_pp_num,
            avg_map_len: sec_to_minsec(result.map_len.avg()),
            accuracy: format!("{:.2}%", user.accuracy),
            pp_per_month: format!("{:.2}pp", pp_per_month_num),
            pp_per_month_num,
            count_ss: with_comma_u64(user.count_ssh as u64 + user.count_ss as u64),
            count_s: with_comma_u64(user.count_sh as u64 + user.count_s as u64),
            count_a: with_comma_u64(user.count_a as u64),
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
    }
}
