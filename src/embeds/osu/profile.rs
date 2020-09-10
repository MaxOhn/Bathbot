use crate::{
    commands::osu::{MinMaxAvgBasic, ProfileResult},
    custom_client::OsuProfile,
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        constants::AVATAR_URL,
        datetime::{date_to_string, how_long_ago, sec_to_minsec},
        numbers::{round, with_comma_int},
        osu::grade_emote,
    },
};

use chrono::Utc;
use rosu::models::{GameMode, Grade, User};
use std::{collections::BTreeMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct ProfileEmbed {
    description: Option<String>,
    author: Author,
    thumbnail: ImageSource,
    image: ImageSource,
    footer: Footer,
    fields: Vec<(String, String, bool)>,
}

impl ProfileEmbed {
    pub fn new(
        user: User,
        profile_result: Option<ProfileResult>,
        globals_count: BTreeMap<usize, String>,
        profile: OsuProfile,
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
        let days = (Utc::now() - user.join_date).num_days() as f32;
        let pp_per_month = 30.67 * user.pp_raw / days;
        let mut fields = vec![
            (
                "Ranked score".to_owned(),
                with_comma_int(user.ranked_score),
                true,
            ),
            (
                "Accuracy".to_owned(),
                format!("{:.2}%", user.accuracy),
                true,
            ),
            (
                "Max combo".to_owned(),
                with_comma_int(profile.statistics.max_combo),
                true,
            ),
            (
                "Total score".to_owned(),
                with_comma_int(user.total_score),
                true,
            ),
            ("Level".to_owned(), format!("{:.2}", user.level), true),
            (
                "PP / month".to_owned(),
                format!("{:.2}pp", pp_per_month),
                true,
            ),
            (
                "Total hits".to_owned(),
                with_comma_int(user.total_hits()),
                true,
            ),
            ("Bonus PP".to_owned(), format!("{}pp", bonus_pp), true),
            (
                "Followers".to_owned(),
                with_comma_int(profile.follower_count),
                true,
            ),
            (
                "Grades".to_owned(),
                format!(
                    "{}{} {}{} {}{} {}{} {}{}",
                    grade_emote(Grade::XH),
                    user.count_ssh,
                    grade_emote(Grade::X),
                    user.count_ss,
                    grade_emote(Grade::SH),
                    user.count_sh,
                    grade_emote(Grade::S),
                    user.count_s,
                    grade_emote(Grade::A),
                    user.count_a,
                ),
                false,
            ),
            (
                "Play count / time".to_owned(),
                format!(
                    "{} / {} hrs",
                    with_comma_int(user.playcount as u64),
                    user.total_seconds_played / 3600
                ),
                true,
            ),
            (
                "Replays watched".to_owned(),
                with_comma_int(profile.statistics.replays_watched),
                true,
            ),
        ];
        let description = if let Some(values) = profile_result {
            let mut avg_string = String::with_capacity(256);
            avg_string.push_str("```\n");
            let _ = writeln!(avg_string, "   |   PP   |  Acc  | Combo | Map len");
            let _ = writeln!(avg_string, "---+--------+-------+-------+--------");
            let _ = writeln!(
                avg_string,
                "Min| {:^6.2} | {:^5} | {:^5} | {:^7}",
                values.pp.min(),
                round(values.acc.min()),
                values.combo.min(),
                sec_to_minsec(values.map_len.min())
            );
            let _ = writeln!(
                avg_string,
                "Avg| {:^6.2} | {:^5} | {:^5} | {:^7}",
                values.pp.avg(),
                round(values.acc.avg()),
                values.combo.avg(),
                sec_to_minsec(values.map_len.avg())
            );
            let _ = writeln!(
                avg_string,
                "Max| {:^6.2} | {:^5} | {:^5} | {:^7}",
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
            fields.extend(vec![(
                "Averages of top 100 scores".to_owned(),
                avg_string,
                false,
            )]);
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
                fields.push(("Favourite mod combinations".to_owned(), value, false));
            }
            fields.reserve_exact(5);
            let len = values.mods_count.len();
            let mut value = String::with_capacity(len * 14);
            let mut iter = values.mods_count.iter();
            let (mods, count) = iter.next().unwrap();
            let _ = write!(value, "`{} {}%`", mods, count);
            for (mods, count) in iter {
                let _ = write!(value, " > `{} {}%`", mods, count);
            }
            fields.push(("Favourite mods".to_owned(), value, false));
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
            fields.push((name.to_owned(), value, false));
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
            fields.push(("Mappers in top 100".to_owned(), value, true));
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
            fields.push(("Global leaderboards".to_owned(), count_str, true));
            None
        } else {
            Some("No Top scores".to_string())
        };
        Self {
            description,
            fields,
            footer: Footer::new(footer_text),
            author: osu::get_user_author(&user),
            image: ImageSource::attachment("profile_graph.png").unwrap(),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
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
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
}
