use crate::{
    commands::osu::{MinMaxAvgBasic, ProfileResult},
    embeds::{attachment, Author, EmbedBuilder, EmbedData, EmbedFields, Footer},
    util::{
        constants::common_literals::{MANIA, TAIKO},
        datetime::{date_to_string, how_long_ago_text, sec_to_minsec},
        numbers::{round, with_comma_int},
        osu::grade_emote,
    },
};

use rosu_v2::prelude::{GameMode, Grade, User, UserStatistics};
use std::{borrow::Cow, collections::BTreeMap, fmt::Write};
use twilight_model::{channel::embed::EmbedField, id::UserId};

#[derive(Clone)]
pub struct ProfileEmbed {
    author: Author,
    description: Option<String>,
    fields: EmbedFields,
    footer: Footer,
    image: String,
    thumbnail: String,
    title: Option<String>,
}

impl ProfileEmbed {
    pub fn compact(user: &User, max_pp: f32) -> Self {
        let stats = user.statistics.as_ref().unwrap();
        let level = stats.level.float();
        let playtime = stats.playtime / 60 / 60;

        let description = format!(
            "Accuracy: `{:.2}%` • Level: `{level:.2}`\n\
            Playcount: `{}` (`{playtime} hrs`)\n\
            Max pp play: `{max_pp:.2}pp` • Mode: `{}`",
            stats.accuracy,
            with_comma_int(stats.playcount),
            match user.mode {
                GameMode::STD => "osu!",
                GameMode::TKO => TAIKO,
                GameMode::CTB => "catch",
                GameMode::MNA => MANIA,
            }
        );

        Self {
            author: author!(user),
            description: Some(description),
            fields: Vec::new(),
            footer: Footer::new(footer_text(user)),
            image: attachment("profile_graph.png"),
            thumbnail: user.avatar_url.to_owned(),
            title: None,
        }
    }

    pub fn medium(user: &User, bonus_pp: f32, discord_id: Option<UserId>) -> Self {
        let mut title = format!(
            "{} statistics",
            match user.mode {
                GameMode::STD => "osu!",
                GameMode::TKO => "Taiko",
                GameMode::CTB => "CtB",
                GameMode::MNA => "Mania",
            }
        );

        if let Some(user_id) = discord_id {
            let _ = write!(title, " for <@{user_id}>");
        }

        title.push(':');

        let footer_text = footer_text(user);
        let stats = user.statistics.as_ref().unwrap();
        let fields = main_fields(user, stats, bonus_pp);

        Self {
            author: author!(user),
            description: None,
            fields,
            footer: Footer::new(footer_text),
            image: attachment("profile_graph.png"),
            thumbnail: user.avatar_url.to_owned(),
            title: Some(title),
        }
    }

    pub fn full(
        user: &User,
        profile_result: Option<&ProfileResult>,
        globals_count: &BTreeMap<usize, Cow<'static, str>>,
        own_top_scores: usize,
        discord_id: Option<UserId>,
    ) -> Self {
        let mut title = format!(
            "{} statistics",
            match user.mode {
                GameMode::STD => "osu!",
                GameMode::TKO => "Taiko",
                GameMode::CTB => "CtB",
                GameMode::MNA => "Mania",
            }
        );

        if let Some(user_id) = discord_id {
            let _ = write!(title, " for <@{user_id}>");
        }

        title.push(':');

        let footer_text = footer_text(user);
        let stats = user.statistics.as_ref().unwrap();

        let bonus_pp = profile_result
            .as_ref()
            .map_or(0.0, |result| result.bonus_pp);

        let mut fields = main_fields(user, stats, bonus_pp);

        let description = if let Some(values) = profile_result {
            let mut avg_string = String::with_capacity(256);
            avg_string.push_str("```\n");
            let _ = writeln!(avg_string, "   |   PP   |  Acc  | Combo | Map len");
            let _ = writeln!(avg_string, "---+--------+-------+-------+--------");

            let _ = writeln!(
                avg_string,
                "Min|{:^8.2}|{:^7}|{:^7}| {:^7}",
                values.pp.min(),
                round(values.acc.min()),
                values.combo.min(),
                sec_to_minsec(values.map_len.min()).to_string()
            );

            let _ = writeln!(
                avg_string,
                "Avg|{:^8.2}|{:^7}|{:^7}| {:^7}",
                values.pp.avg(),
                round(values.acc.avg()),
                values.combo.avg(),
                sec_to_minsec(values.map_len.avg()).to_string()
            );

            let _ = writeln!(
                avg_string,
                "Max|{:^8.2}|{:^7}|{:^7}| {:^7}",
                values.pp.max(),
                round(values.acc.max()),
                values.combo.max(),
                sec_to_minsec(values.map_len.max()).to_string()
            );

            avg_string.push_str("```");
            let mut combo = String::from(&values.combo.avg().to_string());

            match values.mode {
                GameMode::STD | GameMode::CTB => {
                    let _ = write!(combo, "/{}", values.map_combo);
                }
                _ => {}
            }

            let _ = write!(combo, " [{} - {}]", values.combo.min(), values.combo.max());
            fields.push(field!("Averages of top 100 scores", avg_string, false));

            let mult_mods = values.mod_combs_count.is_some();

            if let Some(mod_combs_count) = values.mod_combs_count.as_ref() {
                let len = mod_combs_count.len();
                let mut value = String::with_capacity(len * 14);
                let mut iter = mod_combs_count.iter();
                let (mods, count) = iter.next().unwrap();
                let _ = write!(value, "`{mods} {count}%`");

                for (mods, count) in iter {
                    let _ = write!(value, " > `{mods} {count}%`");
                }

                fields.push(field!("Favourite mod combinations", value, false));
            }

            fields.reserve_exact(5);
            let len = values.mods_count.len();
            let mut value = String::with_capacity(len * 14);
            let mut iter = values.mods_count.iter();
            let (mods, count) = iter.next().unwrap();
            let _ = write!(value, "`{mods} {count}%`");

            for (mods, count) in iter {
                let _ = write!(value, " > `{mods} {count}%`");
            }

            fields.push(field!("Favourite mods", value, false));
            let len = values.mod_combs_pp.len();
            let mut value = String::with_capacity(len * 15);
            let mut iter = values.mod_combs_pp.iter();
            let (mods, pp) = iter.next().unwrap();
            let _ = write!(value, "`{mods} {pp:.2}pp`");

            for (mods, pp) in iter {
                let _ = write!(value, " > `{mods} {pp:.2}pp`");
            }

            let name = if mult_mods {
                "PP earned with mod combination"
            } else {
                "PP earned with mod"
            };

            fields.push(field!(name, value, false));

            let ranked_count = user.ranked_mapset_count.unwrap() + user.loved_mapset_count.unwrap();

            if ranked_count > 0 {
                let mut mapper_stats = String::with_capacity(64);

                let _ = writeln!(
                    mapper_stats,
                    "`Ranked {}` • `Unranked {}`",
                    user.ranked_mapset_count.unwrap_or(0),
                    user.pending_mapset_count.unwrap_or(0),
                );

                let _ = writeln!(
                    mapper_stats,
                    "`Loved {}` • `Graveyard {}`",
                    user.loved_mapset_count.unwrap_or(0),
                    user.graveyard_mapset_count.unwrap_or(0),
                );

                if own_top_scores > 0 {
                    let _ = writeln!(mapper_stats, "Own maps in top scores: {own_top_scores}");
                }

                fields.push(field!("Mapsets from player", mapper_stats, false));
            }

            let len = values
                .mappers
                .iter()
                .map(|(name, _, _)| name.len() + 12)
                .sum();

            let mut value = String::with_capacity(len);
            let mut iter = values.mappers.iter();
            let (name, count, pp) = iter.next().unwrap();
            let _ = writeln!(value, "{name}: {pp:.2}pp ({count})");

            for (name, count, pp) in iter {
                let _ = writeln!(value, "{name}: {pp:.2}pp ({count})");
            }

            fields.push(field!("Mappers in top 100", value, true));

            let count_len = globals_count
                .iter()
                .fold(0, |max, (_, count)| max.max(count.len()));

            let mut count_str = String::with_capacity(64);
            count_str.push_str("```\n");

            for (rank, count) in globals_count {
                let _ = writeln!(
                    count_str,
                    "Top {:<2}: {:>count_len$}",
                    rank,
                    count,
                    count_len = count_len,
                );
            }

            count_str.push_str("```");
            fields.push(field!("Global leaderboards", count_str, true));

            None
        } else {
            Some("No Top scores".to_owned())
        };

        Self {
            author: author!(user),
            description,
            fields,
            footer: Footer::new(footer_text),
            image: attachment("profile_graph.png"),
            thumbnail: user.avatar_url.to_owned(),
            title: Some(title),
        }
    }
}

fn footer_text(user: &User) -> String {
    format!(
        "Joined osu! {} ({})",
        date_to_string(&user.join_date),
        how_long_ago_text(&user.join_date),
    )
}

fn main_fields(user: &User, stats: &UserStatistics, bonus_pp: f32) -> Vec<EmbedField> {
    let level = stats.level.float();

    vec![
        field!(
            "Ranked score",
            with_comma_int(stats.ranked_score).to_string(),
            true
        ),
        field!("Accuracy", format!("{:.2}%", stats.accuracy), true),
        field!(
            "Max combo",
            with_comma_int(stats.max_combo).to_string(),
            true
        ),
        field!(
            "Total score",
            with_comma_int(stats.total_score).to_string(),
            true
        ),
        field!("Level", format!("{:.2}", level), true),
        field!(
            "Medals",
            format!("{}", user.medals.as_ref().unwrap().len()),
            true
        ),
        field!(
            "Total hits",
            with_comma_int(stats.total_hits).to_string(),
            true
        ),
        field!("Bonus PP", format!("{bonus_pp}pp"), true),
        field!(
            "Followers",
            with_comma_int(user.follower_count.unwrap_or(0)).to_string(),
            true
        ),
        field!(
            "Grades",
            format!(
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
            ),
            false
        ),
        field!(
            "Play count / time",
            format!(
                "{} / {} hrs",
                with_comma_int(stats.playcount),
                stats.playtime / 60 / 60
            ),
            true
        ),
        field!(
            "Replays watched",
            with_comma_int(stats.replays_watched).to_string(),
            true
        ),
    ]
}

impl EmbedData for ProfileEmbed {
    fn as_builder(&self) -> EmbedBuilder {
        let mut builder = EmbedBuilder::new()
            .author(&self.author)
            .fields(self.fields.clone())
            .footer(&self.footer)
            .image(&self.image)
            .thumbnail(&self.thumbnail);

        if let Some(ref description) = self.description {
            builder = builder.description(description);
        }

        if let Some(ref title) = self.title {
            builder = builder.title(title);
        }

        builder
    }
}
