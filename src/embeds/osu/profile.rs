use crate::{
    commands::osu::{MinMaxAvgBasic, ProfileResult},
    embeds::{attachment, Author, EmbedBuilder, EmbedData, EmbedFields, Footer},
    util::{
        constants::AVATAR_URL,
        datetime::{date_to_string, how_long_ago, sec_to_minsec},
        numbers::{round, with_comma_uint},
        osu::grade_emote,
    },
};

use rosu_v2::prelude::{GameMode, Grade, User};
use std::{borrow::Cow, collections::BTreeMap, fmt::Write};

#[derive(Clone)]
pub struct ProfileEmbed {
    description: Option<String>,
    author: Author,
    thumbnail: String,
    title: String,
    image: String,
    footer: Footer,
    main_fields: EmbedFields,
    extended_fields: Option<EmbedFields>,
}

impl ProfileEmbed {
    pub fn new(
        user: &User,
        profile_result: Option<ProfileResult>,
        globals_count: BTreeMap<usize, Cow<'static, str>>,
        own_top_scores: usize,
        mode: GameMode,
    ) -> Self {
        let title = format!(
            "{} statistics:",
            match mode {
                GameMode::STD => "osu!",
                GameMode::TKO => "Taiko",
                GameMode::CTB => "CtB",
                GameMode::MNA => "Mania",
            }
        );

        let footer_text = format!(
            "Joined osu! {} ({})",
            date_to_string(&user.join_date),
            how_long_ago(&user.join_date),
        );

        let stats = user.statistics.as_ref().unwrap();

        let bonus_pow = 0.9994_f64.powi(
            (stats.grade_counts.ssh
                + stats.grade_counts.ss
                + stats.grade_counts.sh
                + stats.grade_counts.s
                + stats.grade_counts.a) as i32,
        );

        let bonus_pp = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;

        let main_fields = vec![
            field!(
                "Ranked score",
                with_comma_uint(stats.ranked_score).to_string(),
                true
            ),
            field!("Accuracy", format!("{:.2}%", stats.accuracy), true),
            field!(
                "Max combo",
                with_comma_uint(stats.max_combo).to_string(),
                true
            ),
            field!(
                "Total score",
                with_comma_uint(stats.total_score).to_string(),
                true
            ),
            field!("Level", format!("{:.2}", stats.level.current), true),
            field!(
                "Medals",
                format!("{}", user.medals.as_ref().unwrap().len()),
                true
            ),
            field!(
                "Total hits",
                with_comma_uint(stats.total_hits).to_string(),
                true
            ),
            field!("Bonus PP", format!("{}pp", bonus_pp), true),
            field!(
                "Followers",
                with_comma_uint(user.follower_count.unwrap_or(0)).to_string(),
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
                    with_comma_uint(stats.playcount).to_string(),
                    stats.playtime / 3600
                ),
                true
            ),
            field!(
                "Replays watched",
                with_comma_uint(stats.replays_watched).to_string(),
                true
            ),
        ];

        let (description, extended_fields) = if let Some(values) = profile_result {
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
                sec_to_minsec(values.map_len.min())
            );

            let _ = writeln!(
                avg_string,
                "Avg|{:^8.2}|{:^7}|{:^7}| {:^7}",
                values.pp.avg(),
                round(values.acc.avg()),
                values.combo.avg(),
                sec_to_minsec(values.map_len.avg())
            );

            let _ = writeln!(
                avg_string,
                "Max|{:^8.2}|{:^7}|{:^7}| {:^7}",
                values.pp.max(),
                round(values.acc.max()),
                values.combo.max(),
                sec_to_minsec(values.map_len.max())
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
            let mut extended_fields = vec![field!("Averages of top 100 scores", avg_string, false)];

            let mult_mods = values.mod_combs_count.is_some();

            if let Some(mod_combs_count) = values.mod_combs_count {
                let len = mod_combs_count.len();
                let mut value = String::with_capacity(len * 14);
                let mut iter = mod_combs_count.iter();
                let (mods, count) = iter.next().unwrap();
                let _ = write!(value, "`{} {}%`", mods, count);

                for (mods, count) in iter {
                    let _ = write!(value, " > `{} {}%`", mods, count);
                }

                extended_fields.push(field!("Favourite mod combinations", value, false));
            }

            extended_fields.reserve_exact(5);
            let len = values.mods_count.len();
            let mut value = String::with_capacity(len * 14);
            let mut iter = values.mods_count.iter();
            let (mods, count) = iter.next().unwrap();
            let _ = write!(value, "`{} {}%`", mods, count);

            for (mods, count) in iter {
                let _ = write!(value, " > `{} {}%`", mods, count);
            }

            extended_fields.push(field!("Favourite mods", value, false));
            let len = values.mod_combs_pp.len();
            let mut value = String::with_capacity(len * 15);
            let mut iter = values.mod_combs_pp.iter();
            let (mods, pp) = iter.next().unwrap();
            let _ = write!(value, "`{} {:.2}pp`", mods, *pp);

            for (mods, pp) in iter {
                let _ = write!(value, " > `{} {:.2}pp`", mods, *pp);
            }

            let name = if mult_mods {
                "PP earned with mod combination"
            } else {
                "PP earned with mod"
            };

            extended_fields.push(field!(name, value, false));

            let ranked_count = user.ranked_and_approved_beatmapset_count.unwrap()
                + user.loved_beatmapset_count.unwrap();

            if ranked_count > 0 {
                let mut mapper_stats = String::with_capacity(64);

                let _ = writeln!(
                    mapper_stats,
                    "`Ranked {}` • `Unranked {}`",
                    user.ranked_and_approved_beatmapset_count.unwrap(),
                    user.unranked_beatmapset_count.unwrap(),
                );

                let _ = writeln!(
                    mapper_stats,
                    "`Loved {}` • `Graveyard {}`",
                    user.loved_beatmapset_count.unwrap(),
                    user.graveyard_beatmapset_count.unwrap(),
                );

                if own_top_scores > 0 {
                    let _ = writeln!(mapper_stats, "Own maps in top scores: {}", own_top_scores);
                }

                extended_fields.push(field!("Mapsets from player", mapper_stats, false));
            }

            let len = values
                .mappers
                .iter()
                .map(|(name, _, _)| name.len() + 12)
                .sum();

            let mut value = String::with_capacity(len);
            let mut iter = values.mappers.iter();
            let (name, count, pp) = iter.next().unwrap();
            let _ = writeln!(value, "{}: {:.2}pp ({})", name, *pp, count);

            for (name, count, pp) in iter {
                let _ = writeln!(value, "{}: {:.2}pp ({})", name, *pp, count);
            }

            extended_fields.push(field!("Mappers in top 100", value, true));

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
            extended_fields.push(field!("Global leaderboards", count_str, true));

            (None, Some(extended_fields))
        } else {
            (Some("No Top scores".to_owned()), None)
        };

        Self {
            title,
            main_fields,
            extended_fields,
            description,
            footer: Footer::new(footer_text),
            author: author!(user),
            image: attachment("profile_graph.png"),
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
        }
    }

    pub fn expand(&self) -> EmbedBuilder {
        let mut fields = self.main_fields.clone();

        if let Some(ref extended_fields) = self.extended_fields {
            fields.append(&mut extended_fields.clone())
        }

        let builder = EmbedBuilder::new()
            .author(&self.author)
            .footer(&self.footer)
            .image(&self.image)
            .thumbnail(&self.thumbnail)
            .title(&self.title);

        if let Some(ref description) = self.description {
            builder.description(description)
        } else {
            builder
        }
    }
}

impl EmbedData for ProfileEmbed {
    fn as_builder(&self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .author(&self.author)
            .fields(self.main_fields.clone())
            .footer(&self.footer)
            .image(&self.image)
            .thumbnail(&self.thumbnail)
            .title(&self.title);

        if let Some(ref description) = self.description {
            builder.description(description)
        } else {
            builder
        }
    }

    fn into_builder(self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .author(self.author)
            .fields(self.main_fields)
            .footer(self.footer)
            .image(self.image)
            .thumbnail(self.thumbnail)
            .title(self.title);

        if let Some(description) = self.description {
            builder.description(description)
        } else {
            builder
        }
    }
}
