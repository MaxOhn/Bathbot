use std::{
    fmt::{Display, Write},
    sync::Arc,
};

use bathbot_model::{
    rkyv_util::time::DateTimeRkyv,
    rosu_v2::user::{User, UserHighestRank},
};
use bathbot_util::{
    datetime::{HowLongAgoText, SecToMinSec, NAIVE_DATETIME_FORMAT},
    fields,
    numbers::{round, MinMaxAvg, Number, WithComma},
    osu::BonusPP,
    EmbedBuilder, FooterBuilder, MessageOrigin,
};
use eyre::Result;
use futures::future::BoxFuture;
use rkyv::{
    with::{DeserializeWith, Map},
    Infallible,
};
use rosu_v2::prelude::{GameModIntermode, GameMode, GameModsIntermode, Grade, Score};
use time::UtcOffset;
use twilight_model::{
    channel::message::{
        component::{ActionRow, SelectMenu, SelectMenuOption},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use self::{
    availability::{Availability, MapperNames, ScoreRank, SkinUrl},
    top100_mappers::Top100Mappers,
    top100_mods::Top100Mods,
    top100_stats::Top100Stats,
};
use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::osu::ProfileKind,
    core::Context,
    manager::redis::RedisData,
    util::{interaction::InteractionComponent, osu::grade_emote, Authored, ComponentExt, Emote},
};

mod availability;
mod top100_mappers;
mod top100_mods;
mod top100_stats;

pub struct ProfileMenu {
    user: RedisData<User>,
    discord_id: Option<Id<UserMarker>>,
    tz: Option<UtcOffset>,
    skin_url: Availability<SkinUrl>,
    scores: Availability<Box<[Score]>>,
    score_rank: Availability<ScoreRank>,
    top100stats: Option<Top100Stats>,
    mapper_names: Availability<MapperNames>,
    kind: ProfileKind,
    origin: MessageOrigin,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for ProfileMenu {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        match self.kind {
            ProfileKind::Compact => Box::pin(self.compact(ctx)),
            ProfileKind::UserStats => Box::pin(self.user_stats(ctx)),
            ProfileKind::Top100Stats => Box::pin(self.top100_stats(ctx)),
            ProfileKind::Top100Mods => Box::pin(self.top100_mods(ctx)),
            ProfileKind::Top100Mappers => Box::pin(self.top100_mappers(ctx)),
            ProfileKind::MapperStats => Box::pin(self.mapper_stats(ctx)),
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
            options,
            placeholder: None,
        };

        let components = vec![Component::SelectMenu(menu)];

        vec![Component::ActionRow(ActionRow { components })]
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        async fn inner(
            ctx: Arc<Context>,
            component: &mut InteractionComponent,
            kind: &mut ProfileKind,
            msg_owner: Id<UserMarker>,
        ) -> ComponentResult {
            let user_id = match component.user_id() {
                Ok(user_id) => user_id,
                Err(err) => return ComponentResult::Err(err),
            };

            if user_id != msg_owner {
                return ComponentResult::Ignore;
            }

            let value = component.data.values.pop();

            *kind = match value.as_deref() {
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

            if let Err(err) = component.defer(&ctx).await {
                warn!(?err, "Failed to defer component");
            }

            ComponentResult::BuildPage
        }

        Box::pin(inner(ctx, component, &mut self.kind, self.msg_owner))
    }
}

impl ProfileMenu {
    pub fn new(
        user: RedisData<User>,
        discord_id: Option<Id<UserMarker>>,
        tz: Option<UtcOffset>,
        kind: ProfileKind,
        origin: MessageOrigin,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            user,
            discord_id,
            tz,
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

    async fn compact(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let user_id = self.user.user_id();
        let skin_url = self.skin_url.get(&ctx, user_id).await;

        let (mode, medals, highest_rank) = match self.user {
            RedisData::Original(ref user) => {
                let mode = user.mode;
                let medals = user.medals.len();
                let highest_rank = user.highest_rank.as_ref().cloned();

                (mode, medals, highest_rank)
            }
            RedisData::Archive(ref user) => {
                let mode = user.mode;
                let medals = user.medals.len();
                let highest_rank =
                    Map::<UserHighestRank>::deserialize_with(&user.highest_rank, &mut Infallible)
                        .unwrap();

                (mode, medals, highest_rank)
            }
        };

        let stats = self.user.stats();

        let mut description = format!(
            "Accuracy: [`{acc:.2}%`]({origin} \"{acc}\") • Level: `{level:.2}`\n\
            Playcount: `{playcount}` (`{playtime} hrs`)\n\
            {mode} • Medals: `{medals}`",
            acc = stats.accuracy(),
            origin = self.origin,
            level = stats.level().float(),
            playcount = WithComma::new(stats.playcount()),
            playtime = stats.playtime() / 60 / 60,
            mode = Emote::from(mode).text(),
        );

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
            .author(self.user.author_builder())
            .description(description)
            .footer(self.footer())
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true))
    }

    async fn user_stats(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let bonus_pp = match self.bonus_pp(&ctx).await {
            Some(pp) => format!("{pp:.2}pp"),
            None => "-".to_string(),
        };

        let user_id = self.user.user_id();
        let mode = self.user.mode();

        let score_rank = match self.score_rank.get(&ctx, user_id, mode).await {
            Some(rank) => format!("#{}", WithComma::new(rank)),
            None => "-".to_string(),
        };

        let stats = self.user.stats().to_owned();

        let (highest_rank, medals, follower_count, badges, scores_first_count) = match self.user {
            RedisData::Original(ref user) => {
                let medals = user.medals.len();
                let follower_count = user.follower_count;
                let highest_rank = user.highest_rank.as_ref().cloned();
                let badges = user.badges.len();
                let scores_first_count = user.scores_first_count;

                (
                    highest_rank,
                    medals,
                    follower_count,
                    badges,
                    scores_first_count,
                )
            }
            RedisData::Archive(ref user) => {
                let medals = user.medals.len();
                let follower_count = user.follower_count;
                let badges = user.badges.len();
                let scores_first_count = user.scores_first_count;

                let highest_rank =
                    Map::<UserHighestRank>::deserialize_with(&user.highest_rank, &mut Infallible)
                        .unwrap();

                (
                    highest_rank,
                    medals,
                    follower_count,
                    badges,
                    scores_first_count,
                )
            }
        };

        let mut description = format!(
            "__**{mode} User statistics",
            mode = Emote::from(mode).text(),
        );

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        let hits_per_play = stats.total_hits as f32 / stats.playcount as f32;

        description.push_str(":**__");

        let peak_rank = match highest_rank {
            Some(peak) => format!(
                "#{rank} ({year}/{month:0>2})",
                rank = WithComma::new(peak.rank),
                year = peak.updated_at.year(),
                month = peak.updated_at.month() as u8,
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

        let playcount_value = format!(
            "{} / {} hrs",
            WithComma::new(stats.playcount),
            stats.playtime / 60 / 60
        );

        // https://github.com/ppy/osu-web/blob/0a41b13acf5f47bb0d2b08bab42a9646b7ab5821/app/Models/UserStatistics/Model.php#L84
        let recommended_stars = if stats.pp.abs() <= f32::EPSILON {
            1.0
        } else {
            match mode {
                GameMode::Osu | GameMode::Catch | GameMode::Mania => stats.pp.powf(0.4) * 0.195,
                GameMode::Taiko => stats.pp.powf(0.35) * 0.27,
            }
        };

        let fields = fields![
            "Ranked score", WithComma::new(stats.ranked_score).to_string(), true;
            "Max combo", WithComma::new(stats.max_combo).to_string(), true;
            "Accuracy", format!("[{acc:.2}%]({origin} \"{acc}\")", acc = stats.accuracy, origin = self.origin), true;
            "Total score", WithComma::new(stats.total_score).to_string(), true;
            "Score rank", score_rank, true;
            "Level", format!("{:.2}", stats.level.float()), true;
            "Peak rank", peak_rank, true;
            "Bonus PP", bonus_pp, true;
            "Followers", WithComma::new(follower_count).to_string(), true;
            "Hits per play", WithComma::new(hits_per_play).to_string(), true;
            "Total hits", WithComma::new(stats.total_hits).to_string(), true;
            "Medals", medals.to_string(), true;
            "Recommended", format!("{}★", round(recommended_stars)), true;
            "First places", scores_first_count.to_string(), true;
            "Badges", badges.to_string(), true;
            "Grades", grades_value, false;
            "Play count / time", playcount_value, true;
            "Replays watched", WithComma::new(stats.replays_watched).to_string(), true;
        ];

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .fields(fields)
            .footer(self.footer())
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true))
    }

    async fn top100_stats(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let mode = self.user.mode();
        let mut description = String::with_capacity(1024);

        let _ = write!(
            description,
            "__**{mode} Top100 statistics",
            mode = Emote::from(mode).text(),
        );

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        if let Some(stats) = Top100Stats::prepare(&ctx, self).await {
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
            .author(self.user.author_builder())
            .description(description)
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true))
    }

    async fn top100_mods(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let mode = self.user.mode();
        let mut description = format!("__**{mode} Top100 mods", mode = Emote::from(mode).text(),);

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        let fields = if let Some(stats) = Top100Mods::prepare(&ctx, self).await {
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
            .author(self.user.author_builder())
            .description(description)
            .fields(fields)
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true))
    }

    async fn top100_mappers(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let mut description = format!(
            "__**{mode} Top100 mappers",
            mode = Emote::from(self.user.mode()).text(),
        );

        if let Some(discord_id) = self.discord_id {
            let _ = write!(description, " for <@{discord_id}>");
        }

        description.push_str(":**__\n");

        if let Some(mappers) = Top100Mappers::prepare(&ctx, self).await {
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
            .author(self.user.author_builder())
            .description(description)
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true))
    }

    async fn mapper_stats(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let own_maps_in_top100 = self.own_maps_in_top100(&ctx).await;

        let (
            mode,
            ranked_count,
            loved_count,
            pending_count,
            graveyard_count,
            guest_count,
            kudosu,
            mapping_followers,
        ) = match self.user {
            RedisData::Original(ref user) => {
                let mode = user.mode;
                let ranked_count = user.ranked_mapset_count;
                let loved_count = user.loved_mapset_count;
                let pending_count = user.pending_mapset_count;
                let graveyard_count = user.graveyard_mapset_count;
                let guest_count = user.guest_mapset_count;
                let kudosu = user.kudosu;
                let mapping_followers = user.mapping_follower_count;

                (
                    mode,
                    ranked_count,
                    loved_count,
                    pending_count,
                    graveyard_count,
                    guest_count,
                    kudosu,
                    mapping_followers,
                )
            }
            RedisData::Archive(ref user) => {
                let mode = user.mode;
                let ranked_count = user.ranked_mapset_count;
                let loved_count = user.loved_mapset_count;
                let pending_count = user.pending_mapset_count;
                let graveyard_count = user.graveyard_mapset_count;
                let guest_count = user.guest_mapset_count;
                let kudosu = user.kudosu;
                let mapping_followers = user.mapping_follower_count;

                (
                    mode,
                    ranked_count,
                    loved_count,
                    pending_count,
                    graveyard_count,
                    guest_count,
                    kudosu,
                    mapping_followers,
                )
            }
        };

        let mut description = format!(
            "__**{mode} Mapper statistics",
            mode = Emote::from(mode).text(),
        );

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
            .author(self.user.author_builder())
            .description(description)
            .fields(fields)
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true))
    }

    async fn bonus_pp(&mut self, ctx: &Context) -> Option<f32> {
        let user_id = self.user.user_id();
        let mode = self.user.mode();
        let scores = self.scores.get(ctx, user_id, mode).await?;

        let mut bonus_pp = BonusPP::new();

        for (i, score) in scores.iter().enumerate() {
            if let Some(weight) = score.weight {
                bonus_pp.update(weight.pp, i);
            }
        }

        Some(bonus_pp.calculate(self.user.stats()))
    }

    async fn own_maps_in_top100(&mut self, ctx: &Context) -> Option<usize> {
        let user_id = self.user.user_id();
        let mode = self.user.mode();
        let scores = self.scores.get(ctx, user_id, mode).await?;

        let count = scores.iter().fold(0, |count, score| {
            let self_mapped = score
                .map
                .as_ref()
                .map_or(0, |map| (map.creator_id == user_id) as usize);

            count + self_mapped
        });

        Some(count)
    }

    fn footer(&self) -> FooterBuilder {
        let mut join_date = match self.user {
            RedisData::Original(ref user) => user.join_date,
            RedisData::Archive(ref user) => {
                DateTimeRkyv::deserialize_with(&user.join_date, &mut Infallible).unwrap()
            }
        };

        if let Some(tz) = self.tz {
            join_date = join_date.to_offset(tz);
        }

        let text = format!(
            "Joined osu! {} ({})",
            join_date.format(NAIVE_DATETIME_FORMAT).unwrap(),
            HowLongAgoText::new(&join_date),
        );

        FooterBuilder::new(text)
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
