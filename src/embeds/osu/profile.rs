use crate::{
    commands::osu::ProfileResult,
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        datetime::{date_to_string, how_long_ago, sec_to_minsec},
        globals::AVATAR_URL,
        numbers::{round, with_comma_u64},
        osu::grade_emote,
    },
};

use itertools::Itertools;
use rosu::models::{GameMode, Grade, User};
use serenity::cache::Cache;
use std::{collections::BTreeMap, fmt::Write};

#[derive(Clone)]
pub struct ProfileEmbed {
    description: Option<String>,
    author: Author,
    thumbnail: String,
    footer: Footer,
    fields: Vec<(String, String, bool)>,
}

impl ProfileEmbed {
    pub async fn new(
        user: User,
        profile_result: Option<ProfileResult>,
        globals_count: BTreeMap<usize, String>,
        cache: &Cache,
    ) -> Self {
        let footer_text = format!(
            "Joined osu! {} ({})",
            date_to_string(&user.join_date),
            how_long_ago(&user.join_date),
        );
        let bonus_pow = 0.9994_f64.powi(
            (user.count_ssh + user.count_ss + user.count_sh + user.count_s + user.count_a) as i32,
        );
        let bonus_pp = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;
        let mut fields = vec![
            (
                "Ranked score:".to_owned(),
                with_comma_u64(user.ranked_score),
                true,
            ),
            (
                "Total score:".to_owned(),
                with_comma_u64(user.total_score),
                true,
            ),
            (
                "Total hits:".to_owned(),
                with_comma_u64(user.total_hits()),
                true,
            ),
            (
                "Play count / time:".to_owned(),
                format!(
                    "{} / {} hrs",
                    with_comma_u64(user.playcount as u64),
                    user.total_seconds_played / 3600
                ),
                true,
            ),
            ("Level:".to_owned(), round(user.level).to_string(), true),
            ("Bonus PP:".to_owned(), format!("{}pp", bonus_pp), true),
            (
                "Accuracy:".to_owned(),
                format!("{}%", round(user.accuracy)),
                true,
            ),
        ];
        let description = if let Some(values) = profile_result {
            let mut combo = String::from(&values.avg_combo.to_string());
            match values.mode {
                GameMode::STD | GameMode::CTB => {
                    let _ = write!(combo, "/{}", values.map_combo);
                }
                _ => {}
            }
            let _ = write!(combo, " [{} - {}]", values.min_combo, values.max_combo);
            fields.extend(vec![
                (
                    "Unweighted accuracy:".to_owned(),
                    format!(
                        "{}% [{}% - {}%]",
                        round(values.avg_acc),
                        round(values.min_acc),
                        round(values.max_acc)
                    ),
                    true,
                ),
                (
                    "Grades:".to_owned(),
                    format!(
                        "{}{} {}{} {}{} {}{} {}{}",
                        grade_emote(Grade::XH, cache).await,
                        user.count_ssh,
                        grade_emote(Grade::X, cache).await,
                        user.count_ss,
                        grade_emote(Grade::SH, cache).await,
                        user.count_sh,
                        grade_emote(Grade::S, cache).await,
                        user.count_s,
                        grade_emote(Grade::A, cache).await,
                        user.count_a,
                    ),
                    false,
                ),
                (
                    "Average PP:".to_owned(),
                    format!(
                        "{}pp [{} - {}]",
                        round(values.avg_pp),
                        round(values.min_pp),
                        round(values.max_pp)
                    ),
                    true,
                ),
                ("Average Combo:".to_owned(), combo, true),
            ]);
            if let Some(mod_combs_count) = values.mod_combs_count {
                fields.push((
                    "Favourite mod combinations:".to_owned(),
                    mod_combs_count
                        .into_iter()
                        .map(|(mods, count)| format!("`{} {}%`", mods, count))
                        .join(" > "),
                    false,
                ));
            }
            fields.reserve(if values.mod_combs_pp.is_some() { 6 } else { 5 });
            fields.push((
                "Favourite mods:".to_owned(),
                values
                    .mods_count
                    .into_iter()
                    .map(|(mods, count)| format!("`{} {}%`", mods, count))
                    .join(" > "),
                false,
            ));
            if let Some(mod_combs_pp) = values.mod_combs_pp {
                fields.push((
                    "PP earned with mod combination:".to_owned(),
                    mod_combs_pp
                        .into_iter()
                        .map(|(mods, pp)| format!("`{} {}pp`", mods, round(pp)))
                        .join(" > "),
                    false,
                ));
            }
            fields.push((
                "PP earned with mod:".to_owned(),
                values
                    .mods_pp
                    .into_iter()
                    .map(|(mods, pp)| format!("`{} {}pp`", mods, round(pp)))
                    .join(" > "),
                false,
            ));
            fields.push((
                "Mappers in top 100:".to_owned(),
                values
                    .mappers
                    .into_iter()
                    .map(|(name, count, pp)| format!("{}: {}pp ({})", name, round(pp), count))
                    .join("\n"),
                true,
            ));
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
            fields.push(("Global leaderboard count".to_owned(), count_str, true));
            fields.push((
                "Average map length:".to_owned(),
                format!(
                    "{} [{} - {}]",
                    sec_to_minsec(values.avg_len),
                    sec_to_minsec(values.min_len),
                    sec_to_minsec(values.max_len)
                ),
                false,
            ));
            None
        } else {
            Some("No Top scores".to_string())
        };
        Self {
            description,
            fields,
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            footer: Footer::new(footer_text),
            author: osu::get_user_author(&user),
        }
    }
}

impl EmbedData for ProfileEmbed {
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
