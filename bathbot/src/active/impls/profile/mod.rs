use std::fmt::{Display, Write};

use bathbot_model::RankAccPeaks;
use bathbot_util::{
    Authored, EmbedBuilder, FooterBuilder, MessageOrigin,
    constants::OSU_BASE,
    datetime::{HowLongAgoText, NAIVE_DATETIME_FORMAT, SecToMinSec},
    fields,
    numbers::{MinMaxAvg, Number, WithComma, round},
    osu::{BonusPP, total_score_to_reach_level},
};
use eyre::Result;
use rkyv::rancor::{Panic, ResultExt};
use rosu_v2::prelude::{
    GameModIntermode, GameMode, GameModsIntermode, Grade, Score,
    UserHighestRank as RosuUserHighestRank, UserKudosu,
};
use time::UtcOffset;
use twilight_model::{
    channel::message::{
        Component,
        component::{ActionRow, SelectMenu, SelectMenuOption, SelectMenuType},
    },
    id::{Id, marker::UserMarker},
};

use self::{
    availability::{Availability, MapperNames, ScoreData, SkinUrl},
    top100_mappers::Top100Mappers,
    top100_mods::Top100Mods,
    top100_stats::Top100Stats,
};
use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::osu::ProfileKind,
    manager::redis::osu::CachedUser,
    util::{
        CachedUserExt, ComponentExt, Emote, interaction::InteractionComponent, osu::grade_emote,
    },
};

mod availability;
mod top100_mappers;
mod top100_mods;
mod top100_stats;

pub struct ProfileMenu {
    user: CachedUser,
    discord_id: Option<Id<UserMarker>>,
    tz: Option<UtcOffset>,
    legacy_scores: bool,
    skin_url: Availability<SkinUrl>,
    scores: Availability<Box<[Score]>>,
    score_rank: Availability<ScoreData>,
    osutrack_peaks: Option<RankAccPeaks>,
    top100stats: Option<Top100Stats>,
    mapper_names: Availability<MapperNames>,
    kind: ProfileKind,
    origin: MessageOrigin,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for ProfileMenu {
    async fn build_page(&mut self) -> Result<BuildPage> {
        match self.kind {
            ProfileKind::Compact => self.compact().await,
            ProfileKind::UserStats => self.user_stats().await,
            ProfileKind::Top100Stats => self.top100_stats().await,
            ProfileKind::Top100Mods => self.top100_mods().await,
            ProfileKind::Top100Mappers => self.top100_mappers().await,
            ProfileKind::MapperStats => self.mapper_stats().await,
        }
    }

    fn build_components(&self) -> Vec<Component> {
        let options = vec![
            SelectMenuOption {
                default: matches!(self.kind, ProfileKind::Compact),
                description: Some("Compact user statistics".to_owned()),
                emoji: None,
                label: "Compact".to_owned(),
                value: "compact".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.kind, ProfileKind::UserStats),
                description: Some("Extended user statistics".to_owned()),
                emoji: None,
                label: "User Statistics".to_owned(),
                value: "user_stats".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.kind, ProfileKind::Top100Stats),
                description: Some("Min-Avg-Max values for top100 scores".to_owned()),
                emoji: None,
                label: "Top100 Statistics".to_owned(),
                value: "top100_stats".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.kind, ProfileKind::Top100Mods),
                description: Some("Favourite mods in top100 scores".to_owned()),
                emoji: None,
                label: "Top100 Mods".to_owned(),
                value: "top100_mods".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.kind, ProfileKind::Top100Mappers),
                description: Some("Mapper appearances in top100 scores".to_owned()),
                emoji: None,
                label: "Top100 Mappers".to_owned(),
                value: "top100_mappers".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.kind, ProfileKind::MapperStats),
                description: Some("Mapping statistics & Kudosu".to_owned()),
                emoji: None,
                label: "Mapper Statistics".to_owned(),
                value: "mapper_stats".to_owned(),
            },
        ];

        let menu = SelectMenu {
            custom_id: "profile_menu".to_owned(),
            disabled: false,
            max_values: None,
            min_values: None,
            options: Some(options),
            placeholder: None,
            channel_types: None,
            default_values: None,
            kind: SelectMenuType::Text,
        };

        let components = vec![Component::SelectMenu(menu)];

        vec![Component::ActionRow(ActionRow { components })]
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        let value = component.data.values.pop();

        self.kind = match value.as_deref() {
            Some("compact") => ProfileKind::Compact,
            Some("user_stats") => ProfileKind::UserStats,
            Some("top100_stats") => ProfileKind::Top100Stats,
            Some("top100_mods") => ProfileKind::Top100Mods,
            Some("top100_mappers") => ProfileKind::Top100Mappers,
            Some("mapper_stats") => ProfileKind::MapperStats,
            Some(other) => {
                return ComponentResult::Err(eyre!("Unknown profile menu option `{other}`"));
            }
            None => return ComponentResult::Err(eyre!("Missing value for profile menu")),
        };

        if let Err(err) = component.defer().await {
            warn!(?err, "Failed to defer component");
        }

        ComponentResult::BuildPage
    }
}

impl ProfileMenu {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user: CachedUser,
        discord_id: Option<Id<UserMarker>>,
        tz: Option<UtcOffset>,
        osutrack_peaks: Option<RankAccPeaks>,
        legacy_scores: bool,
        kind: ProfileKind,
        origin: MessageOrigin,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            user,
            discord_id,
            tz,
            osutrack_peaks,
            legacy_scores,
            kind,
            msg_owner,
            skin_url: Availability::NotRequested,
            scores: Availability::NotRequested,
            score_rank: Availability::NotRequested,
            mapper_names: Availability::NotRequested,
            origin,
            top100stats: None,
        }
    }

    async fn compact(&mut self) -> Result<BuildPage> {
        let user_id = self.user.user_id.to_native();

        let medals = self.user.medals.len();
        let mut highest_rank = self
            .user
            .highest_rank
            .as_ref()
            .map(|highest_rank| highest_rank.try_deserialize::<Panic>().always_ok());

        let stats = self.user.statistics.as_ref().expect("missing stats");
        let level = stats.level.float();
        let missing_score = missing_score_for_levelup(level, stats.total_score.to_native());

        self.consider_osutrack_peaks(&mut highest_rank);
        let skin_url = self.skin_url.get(user_id).await;

        let mut description = format!(
            "Accuracy: [`{acc:.2}%`]({origin} \"{acc}\") • \
            Level: [`{level:.2}`]({origin} \"Total score until next level: {missing_score}\")\n\
            Playcount: `{playcount}` (`{playtime} hrs`)\n\
            Medals: `{medals}`",
            acc = stats.accuracy.to_native(),
            origin = self.origin,
            missing_score = WithComma::new(missing_score),
            playcount = WithComma::new(stats.playcount.to_native()),
            playtime = stats.playtime.to_native() / 60 / 60,
        );

        if let Some(team) = self.user.team.as_ref() {
            let _ = write!(
                description,
                " • Team [{short_name}]({OSU_BASE}teams/{id} \"{name}\")",
                short_name = team.short_name.as_str(),
                name = team.name.as_str(),
                id = team.id.to_native(),
            );
        }

        if let Some(skin_url) = skin_url {
            let skin_tooltip = skin_url.trim_start_matches("https://");
            let _ = write!(
                description,
                " • [**Link to skin**]({skin_url} \"{skin_tooltip}\")"
            );
        }

        if let Some(peak) = highest_rank {
            let _ = write!(
                description,
                "\nPeak rank: `#{rank}` (<t:{timestamp}:d>)",
                rank = WithComma::new(peak.rank),
                timestamp = peak.updated_at.unix_timestamp()
            );
        }

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(true))
            .description(description)
            .footer(self.footer())
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }

    async fn user_stats(&mut self) -> Result<BuildPage> {
        let user_id = self.user.user_id.to_native();
        let mode = self.user.mode;

        let scores_fut = self.scores.get(user_id, mode, self.legacy_scores);
        let score_rank_fut = self.score_rank.get(user_id, mode);

        let (scores_opt, score_rank_opt) = tokio::join!(scores_fut, score_rank_fut);

        let top_score_pp = match scores_opt {
            Some([_score @ Score { pp: Some(pp), .. }, ..]) => format!("{pp:.2}pp"),
            Some(_) | None => "-".to_string(),
        };

        let stats = self.user.statistics.as_ref().expect("missing stats");
        let level = stats.level.float();
        let missing_score = missing_score_for_levelup(level, stats.total_score.to_native());

        let level = format!(
            "[{level:.2}]({origin} \"Total score until next level: {missing_score}\")",
            origin = self.origin,
            missing_score = WithComma::new(missing_score),
        );

        let bonus_pp = match scores_opt {
            Some(scores) => {
                let mut bonus_pp = BonusPP::new();

                for (i, score) in scores.iter().enumerate() {
                    if let Some(weight) = score.weight {
                        bonus_pp.update(weight.pp, i);
                    }
                }

                let pp = bonus_pp.calculate(self.user.statistics.as_ref().expect("missing stats"));

                format!("{pp:.2}pp")
            }
            None => "-".to_string(),
        };

        let (score_rank, peak_score_rank) = match score_rank_opt {
            Some(data) => {
                let rank = data.rank.map_or_else(
                    || "-".to_string(),
                    |rank| format!("#{}", WithComma::new(rank.get())),
                );

                let peak = data.highest_rank.map_or_else(
                    || "-".to_string(),
                    |peak| {
                        let mut peak_datetime = peak.updated_at;

                        if let Some(offset) = self.tz {
                            peak_datetime = peak_datetime.to_offset(offset);
                        }

                        format!(
                            "#{rank} ('{year:0>2}/{month:0>2})",
                            rank = WithComma::new(peak.rank),
                            year = peak_datetime.year() % 100,
                            month = peak_datetime.month() as u8,
                        )
                    },
                );

                (rank, peak)
            }
            None => ("-".to_string(), "-".to_string()),
        };

        let medals = self.user.medals.len();
        let follower_count = self.user.follower_count;
        let badges = self.user.badges.len();
        let scores_first_count = self.user.scores_first_count;

        let mut highest_rank = self
            .user
            .highest_rank
            .as_ref()
            .map(|highest_rank| highest_rank.try_deserialize::<Panic>().always_ok());

        let mut description = "__**User statistics".to_owned();

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        let hits_per_play =
            stats.total_hits.to_native() as f32 / stats.playcount.to_native() as f32;

        description.push_str(":**__");

        self.consider_osutrack_peaks(&mut highest_rank);

        let peak_rank = match highest_rank {
            Some(peak) => {
                let mut peak_datetime = peak.updated_at;

                if let Some(offset) = self.tz {
                    peak_datetime = peak_datetime.to_offset(offset);
                }

                format!(
                    "#{rank} ('{year:0>2}/{month:0>2})",
                    rank = WithComma::new(peak.rank),
                    year = peak_datetime.year() % 100,
                    month = peak_datetime.month() as u8,
                )
            }
            None => "-".to_string(),
        };

        let peak_acc = match self.osutrack_peaks.as_ref() {
            Some(peaks) => format!(
                "[{acc:.2}%]({origin} \"{acc}%\n\nProvided by ameobea.me/osutrack\") ('{year:0>2}/{month:0>2})",
                acc = peaks.acc.max(stats.accuracy.to_native() as f64),
                origin = self.origin,
                year = peaks.acc_timestamp.year() % 100,
                month = peaks.acc_timestamp.month() as u8,
            ),
            None => "-".to_string(),
        };

        let grades_value = format!(
            "{}{} {}{} {}{} {}{} {}{}",
            grade_emote(Grade::XH),
            stats.grade_counts.ssh,
            grade_emote(Grade::X),
            stats.grade_counts.ss,
            grade_emote(Grade::SH),
            stats.grade_counts.sh,
            grade_emote(Grade::S),
            stats.grade_counts.s,
            grade_emote(Grade::A),
            stats.grade_counts.a,
        );

        let combined_grades_value = format!(
            "{}{} {}{}",
            grade_emote(Grade::X),
            stats.grade_counts.ssh + stats.grade_counts.ss,
            grade_emote(Grade::S),
            stats.grade_counts.sh + stats.grade_counts.s,
        );

        let playcount_value = format!(
            "{} / {} hrs",
            WithComma::new(stats.playcount.to_native()),
            stats.playtime / 60 / 60
        );

        // https://github.com/ppy/osu-web/blob/0a41b13acf5f47bb0d2b08bab42a9646b7ab5821/app/Models/UserStatistics/Model.php#L84
        let recommended_stars = if stats.pp.to_native().abs() <= f32::EPSILON {
            1.0
        } else {
            match mode {
                GameMode::Osu | GameMode::Catch | GameMode::Mania => {
                    stats.pp.to_native().powf(0.4) * 0.195
                }
                GameMode::Taiko => stats.pp.to_native().powf(0.35) * 0.27,
            }
        };

        let fields = fields![
            "Peak rank", peak_rank, true;
            "Top score PP", top_score_pp, true;
            "Level", level, true;
            "Total score", WithComma::new(stats.total_score.to_native()).to_string(), true;
            "Total hits", WithComma::new(stats.total_hits.to_native()).to_string(), true;
            "Bonus PP", bonus_pp, true;
            "Ranked score", WithComma::new(stats.ranked_score.to_native()).to_string(), true;
            "Peak score rank", peak_score_rank, true;
            "Score rank", score_rank, true;
            "Hits per play", WithComma::new(hits_per_play).to_string(), true;
            "Peak accuracy", peak_acc, true;
            "Accuracy", format!("[{acc:.2}%]({origin} \"{acc}%\")", acc = stats.accuracy, origin = self.origin), true;
            "Recommended", format!("{}★", round(recommended_stars)), true;
            "Max combo", WithComma::new(stats.max_combo.to_native()).to_string(), true;
            "Medals", medals.to_string(), true;
            "Combined grades", combined_grades_value, true;
            "First places", scores_first_count.to_string(), true;
            "Badges", badges.to_string(), true;
            "Grades", grades_value, false;
            "Play count / time", playcount_value, true;
            "Replays watched", WithComma::new(stats.replays_watched.to_native()).to_string(), true;
            "Followers", WithComma::new(follower_count.to_native()).to_string(), true;
        ];

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(true))
            .description(description)
            .fields(fields)
            .footer(self.footer())
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }

    async fn top100_stats(&mut self) -> Result<BuildPage> {
        let mode = self.user.mode;
        let mut description = String::with_capacity(1024);

        let _ = write!(
            description,
            "__**{mode} Top100 statistics",
            mode = Emote::from(mode),
        );

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        if let Some(stats) = Top100Stats::prepare(self).await {
            description.push_str("```\n");

            let Top100Stats {
                acc,
                combo,
                misses,
                pp,
                stars,
                ar,
                cs,
                hp,
                od,
                bpm,
                len,
            } = stats;

            fn min_avg_max<T: Number>(
                v: &MinMaxAvg<T>,
                f: fn(T) -> String,
            ) -> (String, String, String) {
                (f(v.min()), f(v.avg()), f(v.max()))
            }

            let combo_min = combo.min().to_string();
            let combo_avg = format!("{:.2}", combo.avg_float());
            let combo_max = combo.max().to_string();

            let misses_min = misses.min().to_string();
            let misses_avg = format!("{:.2}", misses.avg_float());
            let misses_max = misses.max().to_string();

            let (acc_min, acc_avg, acc_max) = min_avg_max(acc, |v| format!("{v:.2}"));
            let (pp_min, pp_avg, pp_max) = min_avg_max(pp, |v| format!("{v:.2}"));
            let (stars_min, stars_avg, stars_max) = min_avg_max(stars, |v| format!("{v:.2}"));
            let (ar_min, ar_avg, ar_max) = min_avg_max(ar, |v| format!("{v:.2}"));
            let (cs_min, cs_avg, cs_max) = min_avg_max(cs, |v| format!("{v:.2}"));
            let (hp_min, hp_avg, hp_max) = min_avg_max(hp, |v| format!("{v:.2}"));
            let (od_min, od_avg, od_max) = min_avg_max(od, |v| format!("{v:.2}"));
            let (bpm_min, bpm_avg, bpm_max) = min_avg_max(bpm, |v| format!("{v:.2}"));
            let (len_min, len_avg, len_max) =
                min_avg_max(len, |v| SecToMinSec::new(v as u32).to_string());

            let min_w = "Minimum"
                .len()
                .max(acc_min.len())
                .max(combo_min.len())
                .max(misses_min.len())
                .max(pp_min.len())
                .max(stars_min.len())
                .max(ar_min.len())
                .max(cs_min.len())
                .max(hp_min.len())
                .max(od_min.len())
                .max(bpm_min.len())
                .max(len_min.len());

            let avg_w = "Average"
                .len()
                .max(acc_avg.len())
                .max(combo_avg.len())
                .max(misses_avg.len())
                .max(pp_avg.len())
                .max(stars_avg.len())
                .max(ar_avg.len())
                .max(cs_avg.len())
                .max(hp_avg.len())
                .max(od_avg.len())
                .max(bpm_avg.len())
                .max(len_avg.len());

            let max_w = "Maximum"
                .len()
                .max(acc_max.len())
                .max(combo_max.len())
                .max(misses_max.len())
                .max(pp_max.len())
                .max(stars_max.len())
                .max(ar_max.len())
                .max(cs_max.len())
                .max(hp_max.len())
                .max(od_max.len())
                .max(bpm_max.len())
                .max(len_max.len());

            let _ = writeln!(
                description,
                "         | {min:^min_w$} | {avg:^avg_w$} | {max:^max_w$}",
                min = "Minimum",
                avg = "Average",
                max = "Maximum"
            );

            let _ = writeln!(
                description,
                "{dash:-^9}+-{dash:-^min_w$}-+-{dash:-^avg_w$}-+-{dash:-^max_w$}",
                dash = "-"
            );

            let _ = writeln!(
                description,
                "Accuracy | {acc_min:^min_w$} | {acc_avg:^avg_w$} | {acc_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "Combo    | {combo_min:^min_w$} | {combo_avg:^avg_w$} | {combo_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "Misses   | {misses_min:^min_w$} | {misses_avg:^avg_w$} | {misses_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "PP       | {pp_min:^min_w$} | {pp_avg:^avg_w$} | {pp_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "Stars    | {stars_min:^min_w$} | {stars_avg:^avg_w$} | {stars_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "AR       | {ar_min:^min_w$} | {ar_avg:^avg_w$} | {ar_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "CS       | {cs_min:^min_w$} | {cs_avg:^avg_w$} | {cs_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "HP       | {hp_min:^min_w$} | {hp_avg:^avg_w$} | {hp_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "OD       | {od_min:^min_w$} | {od_avg:^avg_w$} | {od_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "BPM      | {bpm_min:^min_w$} | {bpm_avg:^avg_w$} | {bpm_max:^max_w$}",
            );

            let _ = writeln!(
                description,
                "Length   | {len_min:^min_w$} | {len_avg:^avg_w$} | {len_max:^max_w$}",
            );

            description.push_str("```");
        } else {
            description.push_str("No top scores :(");
        };

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(true))
            .description(description)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }

    async fn top100_mods(&mut self) -> Result<BuildPage> {
        let mode = self.user.mode;
        let mut description = format!("__**{mode} Top100 mods", mode = Emote::from(mode));

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        let fields = if let Some(stats) = Top100Mods::prepare(self).await {
            fn mod_value<M, V, F, const N: usize>(
                map: &[(M, V)],
                to_string: F,
                suffix: &str,
            ) -> Option<String>
            where
                M: HasLen + Display,
                F: Fn(&V) -> String,
            {
                let mut mods_len = [0; N];
                let mut vals_len = [0; N];

                let collected: Vec<_> = map
                    .iter()
                    .enumerate()
                    .map(|(i, (key, value))| {
                        let value = to_string(value);

                        let i = i % N;
                        mods_len[i] = mods_len[i].max(key.len());
                        vals_len[i] = vals_len[i].max(value.len());

                        (key, value)
                    })
                    .collect();

                let mut iter = collected.iter().enumerate();

                if let Some((_, (mods, val))) = iter.next() {
                    let mut value = String::with_capacity(128);

                    let _ = write!(
                        value,
                        "`{mods}:{val:>0$}{suffix}`",
                        vals_len[0] + (mods_len[0].max(1) - mods.len().max(1)) * 2,
                    );

                    for (mut i, (mods, val)) in iter {
                        i %= N;

                        if i == 0 {
                            value.push('\n');
                        } else {
                            value.push_str(" • ");
                        }

                        let _ = write!(
                            value,
                            "`{mods}:{val:>0$}{suffix}`",
                            vals_len[i] + (mods_len[i].max(1) - mods.len().max(1)) * 2,
                        );
                    }

                    Some(value)
                } else {
                    None
                }
            }

            let mut fields = Vec::with_capacity(3);

            if let Some(val) = mod_value::<_, _, _, 4>(&stats.percent_mods, u8::to_string, "%") {
                fields![fields { "Favourite mods", val, false }];
            }

            if let Some(val) = mod_value::<_, _, _, 3>(&stats.percent_mod_comps, u8::to_string, "%")
            {
                fields![fields { "Favourite mod combinations", val, false }];
            }

            if let Some(val) =
                mod_value::<_, _, _, 3>(&stats.pp_mod_comps, |pp| format!("{pp:.1}"), "")
            {
                fields![fields { "Profitable mod combinations (pp)", val, false }];
            }

            fields
        } else {
            description.push_str("No top scores :(");

            Vec::new()
        };

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(true))
            .description(description)
            .fields(fields)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }

    async fn top100_mappers(&mut self) -> Result<BuildPage> {
        let mut description = format!(
            "__**{mode} Top100 mappers",
            mode = Emote::from(self.user.mode),
        );

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        if let Some(mappers) = Top100Mappers::prepare(self).await {
            description.push_str("```\n");

            let mut names_len = 0;
            let mut pp_len = 2;
            let mut count_len = 1;

            let values: Vec<_> = mappers
                .iter()
                .map(|entry| {
                    let pp = format!("{:.2}", entry.pp);
                    let count = entry.count.to_string();

                    names_len = names_len.max(entry.name.len());
                    pp_len = pp_len.max(pp.len());
                    count_len = count_len.max(count.len());

                    (pp, count)
                })
                .collect();

            let _ = writeln!(
                description,
                "{blank:<names_len$} | {pp:^pp_len$} | {count:^count_len$}",
                blank = " ",
                pp = "PP",
                count = "#",
            );

            let _ = writeln!(
                description,
                "{dash:-<names_len$}-+-{dash:->pp_len$}-+-{dash:->count_len$}-",
                dash = "-",
            );

            for (entry, (pp, count)) in mappers.iter().zip(values) {
                let _ = writeln!(
                    description,
                    "{name:<names_len$} | {pp:>pp_len$} | {count:>count_len$}",
                    name = entry.name,
                );
            }

            description.push_str("```");
        } else {
            description.push_str("No top scores :(");
        }

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(true))
            .description(description)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }

    async fn mapper_stats(&mut self) -> Result<BuildPage> {
        let own_maps_in_top100 = self.own_maps_in_top100().await;

        let mode = self.user.mode;
        let ranked_count = self.user.ranked_mapset_count.to_native();
        let loved_count = self.user.loved_mapset_count.to_native();
        let pending_count = self.user.pending_mapset_count.to_native();
        let graveyard_count = self.user.graveyard_mapset_count.to_native();
        let guest_count = self.user.guest_mapset_count.to_native();
        let kudosu = UserKudosu {
            available: self.user.kudosu.available.to_native(),
            total: self.user.kudosu.total.to_native(),
        };
        let mapping_followers = self.user.mapping_follower_count.to_native();

        let mut description = format!("__**{mode} Mapper statistics", mode = Emote::from(mode));

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        let ranked_count = ranked_count.to_string();
        let loved_count = loved_count.to_string();
        let pending_count = pending_count.to_string();
        let graveyard_count = graveyard_count.to_string();
        let guest_count = guest_count.to_string();

        let left_len = ranked_count
            .len()
            .max(pending_count.len())
            .max(guest_count.len());

        let right_len = loved_count.len().max(graveyard_count.len());

        let mapsets_value = format!(
            "`Ranked:  {ranked_count:>left_len$}`  `Loved:     {loved_count:>right_len$}`\n\
            `Pending: {pending_count:>left_len$}`  `Graveyard: {graveyard_count:>right_len$}`\n\
            `Guest:   {guest_count:>left_len$}`"
        );

        let kudosu_value = format!(
            "`Available: {}` • `Total: {}`",
            kudosu.available, kudosu.total,
        );

        let mut fields = fields![
            "Mapsets", mapsets_value, false;
            "Kudosu", kudosu_value, false;
            "Subscribers", mapping_followers.to_string(), true;
        ];

        if let Some(count) = own_maps_in_top100 {
            fields![fields { "Own maps in top100", count.to_string(), true }];
        }

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(true))
            .description(description)
            .fields(fields)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }

    async fn own_maps_in_top100(&mut self) -> Option<usize> {
        let user_id = self.user.user_id.to_native();
        let mode = self.user.mode;
        let scores = self.scores.get(user_id, mode, self.legacy_scores).await?;

        let count = scores.iter().fold(0, |count, score| {
            let self_mapped = score
                .map
                .as_ref()
                .map_or(0, |map| (map.creator_id == user_id) as usize);

            count + self_mapped
        });

        Some(count)
    }

    fn consider_osutrack_peaks(&self, highest_rank: &mut Option<RosuUserHighestRank>) {
        let Some(ref peaks) = self.osutrack_peaks else {
            return;
        };

        match highest_rank {
            Some(highest_rank) => {
                if peaks.rank < highest_rank.rank && peaks.rank > 0 {
                    debug!(
                        osu = ?(highest_rank.rank, highest_rank.updated_at.date()),
                        osutrack = ?(peaks.rank, peaks.rank_timestamp.date()),
                        "osutrack peak was better"
                    );

                    highest_rank.rank = peaks.rank;
                    highest_rank.updated_at = peaks.rank_timestamp;
                }
            }
            None => {
                *highest_rank = Some(RosuUserHighestRank {
                    rank: peaks.rank,
                    updated_at: peaks.rank_timestamp,
                })
            }
        }
    }

    fn footer(&self) -> FooterBuilder {
        let mut join_date = self.user.join_date.try_deserialize::<Panic>().always_ok();

        if let Some(tz) = self.tz {
            join_date = join_date.to_offset(tz);
        }

        let text = format!(
            "Joined osu! {} ({})",
            join_date.format(NAIVE_DATETIME_FORMAT).unwrap(),
            HowLongAgoText::new(&join_date),
        );

        FooterBuilder::new(text).icon_url(Emote::from(self.user.mode).url())
    }
}

trait HasLen {
    fn len(&self) -> usize;
}

impl HasLen for GameModsIntermode {
    fn len(&self) -> usize {
        self.len()
    }
}

impl HasLen for GameModIntermode {
    fn len(&self) -> usize {
        1
    }
}

fn missing_score_for_levelup(level: f32, total_score: u64) -> u64 {
    total_score_to_reach_level(level.ceil() as u32).saturating_sub(total_score)
}
