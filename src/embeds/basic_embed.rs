use super::util;
use crate::{
    scraper::MostPlayedMap,
    util::{
        discord,
        globals::{AVATAR_URL, HOMEPAGE},
        numbers::{round, round_and_comma, with_comma_u64},
    },
};

use rosu::models::{GameMode, Score, User};
use serenity::{builder::CreateEmbed, utils::Colour};
use std::{collections::HashMap, f32, fmt::Write, u32};

#[derive(Default, Debug, Clone)]
pub struct BasicEmbedData {
    pub author_icon: Option<String>,
    pub author_url: Option<String>,
    pub author_text: Option<String>,
    pub title_text: Option<String>,
    pub title_url: Option<String>,
    pub thumbnail: Option<String>,
    pub footer_text: Option<String>,
    pub footer_icon: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub fields: Option<Vec<(String, String, bool)>>,
}

impl BasicEmbedData {
    // ------------------------
    // BUILD THE FINISHED EMBED
    // ------------------------
    pub fn build(self, e: &mut CreateEmbed) -> &mut CreateEmbed {
        if self.author_icon.is_some() || self.author_url.is_some() || self.author_text.is_some() {
            e.author(|a| {
                if let Some(ref author_icon) = self.author_icon {
                    a.icon_url(author_icon);
                }
                if let Some(ref author_url) = self.author_url {
                    a.url(author_url);
                }
                if let Some(ref author_text) = self.author_text {
                    a.name(author_text);
                }
                a
            });
        }
        if self.footer_text.is_some() || self.footer_icon.is_some() {
            e.footer(|f| {
                if let Some(ref footer_text) = self.footer_text {
                    f.text(footer_text);
                }
                if let Some(ref footer_icon) = self.footer_icon {
                    f.icon_url(footer_icon);
                }
                f
            });
        }
        if let Some(ref title) = self.title_text {
            e.title(title);
        }
        if let Some(ref url) = self.title_url {
            e.url(url);
        }
        if let Some(ref thumbnail) = self.thumbnail {
            e.thumbnail(thumbnail);
        }
        if let Some(ref description) = self.description {
            e.description(description);
        }
        if let Some(ref image_url) = self.image_url {
            e.image(image_url);
        }
        if let Some(fields) = self.fields {
            e.fields(fields);
        }
        e.color(Colour::DARK_GREEN)
    }

    //
    // mostplayedcommon
    //
    /// Returns a tuple containing a new `BasicEmbedData` object,
    /// and a `Vec<u8>` representing the bytes of a png
    pub async fn create_mostplayedcommon(
        users: HashMap<u32, User>,
        mut maps: Vec<MostPlayedMap>,
        users_count: HashMap<u32, HashMap<u32, u32>>,
    ) -> (Self, Vec<u8>) {
        let mut result = Self::default();
        // Sort maps by sum of counts
        let total_counts: HashMap<u32, u32> = users_count.iter().fold(
            HashMap::with_capacity(maps.len()),
            |mut counts, (_, user_entry)| {
                for (map_id, count) in user_entry {
                    *counts.entry(*map_id).or_insert(0) += count;
                }
                counts
            },
        );
        maps.sort_by(|a, b| {
            total_counts
                .get(&b.beatmap_id)
                .unwrap()
                .cmp(total_counts.get(&a.beatmap_id).unwrap())
        });
        // Write msg
        let mut description = String::with_capacity(512);
        for (i, map) in maps.into_iter().enumerate() {
            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id}) [{stars}]",
                idx = i + 1,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                stars = util::get_stars(map.stars),
            );
            let mut top_users: Vec<(u32, u32)> = users_count
                .iter()
                .map(|(user_id, entry)| (*user_id, *entry.get(&map.beatmap_id).unwrap()))
                .collect();
            top_users.sort_by(|a, b| b.1.cmp(&a.1));
            let mut top_users = top_users.into_iter().take(3);
            let (first_name, first_count) = top_users
                .next()
                .map(|(user_id, count)| (&users.get(&user_id).unwrap().username, count))
                .unwrap();
            let (second_name, second_count) = top_users
                .next()
                .map(|(user_id, count)| (&users.get(&user_id).unwrap().username, count))
                .unwrap();
            let _ = write!(
                description,
                "- :first_place: `{}`: **{}** :second_place: `{}`: **{}**",
                first_name, first_count, second_name, second_count
            );
            if let Some((third_id, third_count)) = top_users.next() {
                let third_name = &users.get(&third_id).unwrap().username;
                let _ = write!(
                    description,
                    " :third_place: `{}`: **{}**",
                    third_name, third_count
                );
            }
            description.push('\n');
        }
        // Keys have no strict order, hence inconsistent result
        let user_ids: Vec<u32> = users.keys().copied().collect();
        let thumbnail = discord::get_combined_thumbnail(&user_ids)
            .await
            .unwrap_or_else(|e| {
                warn!("Error while combining avatars: {}", e);
                Vec::default()
            });
        result.description = Some(description);
        (result, thumbnail)
    }

    //
    // pp missing
    //
    pub fn create_ppmissing(user: User, scores: Vec<Score>, pp: f32) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let title = format!(
            "What score is {name} missing to reach {pp_given}pp?",
            name = user.username,
            pp_given = pp
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let description = if scores.is_empty() {
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{pp}pp** score which would be the top #1",
                pp = round(pp),
                user = user.username,
            )
        } else if user.pp_raw > pp {
            format!(
                "{name} already has {pp_raw}pp which is more than {pp_given}pp.\n\
                 No more scores are required.",
                name = user.username,
                pp_raw = round_and_comma(user.pp_raw),
                pp_given = pp
            )
        } else {
            let pp_values: Vec<f32> = scores.into_iter().map(|score| score.pp.unwrap()).collect();
            let size: usize = pp_values.len();
            let mut idx: usize = size - 1;
            let mut factor: f32 = 0.95_f32.powi(idx as i32);
            let mut top: f32 = user.pp_raw;
            let mut bot: f32 = 0.0;
            let mut current: f32 = pp_values[idx];
            while top + bot < pp {
                top -= current * factor;
                if idx == 0 {
                    break;
                }
                current = pp_values[idx - 1];
                bot += current * factor;
                factor /= 0.95;
                idx -= 1;
            }
            let mut required: f32 = pp - top - bot;
            if top + bot >= pp {
                factor *= 0.95;
                required = (required + factor * pp_values[idx]) / factor;
                idx += 1;
            }
            idx += 1;
            if size < 100 {
                required -= pp_values[size - 1] * 0.95_f32.powi(size as i32 - 1);
            }
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{required}pp** score which would be the top #{idx}",
                pp = round(pp),
                user = user.username,
                required = round(required),
                idx = idx
            )
        };
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }

    //
    // rank
    //
    pub fn create_rank(
        user: User,
        scores: Vec<Score>,
        rank: usize,
        country: Option<String>,
        rank_holder: User,
    ) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let country = country.unwrap_or_else(|| '#'.to_string());
        let title = format!(
            "How many pp is {name} missing to reach rank {country}{rank}?",
            name = user.username,
            country = country,
            rank = rank
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let description = if user.pp_raw > rank_holder.pp_raw {
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is with **{pp}pp** already above that.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = round_and_comma(rank_holder.pp_raw),
                name = user.username,
                pp = round_and_comma(user.pp_raw)
            )
        } else if scores.is_empty() {
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is missing **{holder_pp}** raw pp, \
                 achievable by a single score worth **{holder_pp}pp**.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = round_and_comma(rank_holder.pp_raw),
                name = user.username,
            )
        } else {
            let pp_values: Vec<f32> = scores.into_iter().map(|score| score.pp.unwrap()).collect();
            let size: usize = pp_values.len();
            let mut idx: usize = size - 1;
            let mut factor: f32 = 0.95_f32.powi(idx as i32);
            let mut top: f32 = user.pp_raw;
            let mut bot: f32 = 0.0;
            let mut current: f32 = pp_values[idx];
            while top + bot < rank_holder.pp_raw {
                top -= current * factor;
                if idx == 0 {
                    break;
                }
                current = pp_values[idx - 1];
                bot += current * factor;
                factor /= 0.95;
                idx -= 1;
            }
            let mut required: f32 = rank_holder.pp_raw - top - bot;
            if top + bot >= rank_holder.pp_raw {
                factor *= 0.95;
                required = (required + factor * pp_values[idx]) / factor;
            }
            if size < 100 {
                required -= pp_values[size - 1] * 0.95_f32.powi(size as i32 - 1);
            }
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is missing **{missing}** raw pp, \
                 achievable by a single score worth **{pp}pp**.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = round_and_comma(rank_holder.pp_raw),
                name = user.username,
                missing = round_and_comma(rank_holder.pp_raw - user.pp_raw),
                pp = round_and_comma(required),
            )
        };
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }

    //
    //  whatif
    //
    pub fn create_whatif(user: User, scores: Vec<Score>, _mode: GameMode, pp: f32) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let title = format!(
            "What if {name} got a new {pp_given}pp score?",
            name = user.username,
            pp_given = pp
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let pp_values: Vec<f32> = scores
            .iter()
            .map(|score| *score.pp.as_ref().unwrap())
            .collect();
        let description = if scores.is_empty() {
            format!(
                "A {pp}pp play would be {name}'s #1 best play.\n\
                 Their pp would change by **+{pp}** to **{pp}pp**.",
                pp = round(pp),
                name = user.username,
            )
        } else if pp < pp_values[pp_values.len() - 1] {
            format!(
                "A {pp_given}pp play wouldn't even be in {name}'s top 100 plays.\n\
                 There would not be any significant pp change.",
                pp_given = pp,
                name = user.username
            )
        } else {
            let mut actual: f32 = 0.0;
            let mut factor: f32 = 1.0;
            for score in pp_values.iter() {
                actual += score * factor;
                factor *= 0.95;
            }
            let bonus = user.pp_raw - actual;
            let mut potential = 0.0;
            let mut used = false;
            let mut new_pos = None;
            let mut factor = 1.0;
            for (i, pp_value) in pp_values.iter().enumerate().take(pp_values.len() - 1) {
                if !used && *pp_value < pp {
                    used = true;
                    potential += pp * factor;
                    factor *= 0.95;
                    new_pos = Some(i + 1);
                }
                potential += pp_value * factor;
                factor *= 0.95;
            }
            format!(
                "A {pp}pp play would be {name}'s #{num} best play.\n\
                 Their pp would change by **+{pp_change}** to **{new_pp}pp**.",
                pp = round(pp),
                name = user.username,
                num = new_pos.unwrap(),
                pp_change = round(potential + bonus - user.pp_raw),
                new_pp = round(potential + bonus)
            )
        };
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }
}

// -------------------
// Auxiliary functions
// -------------------

fn get_user_author(user: &User) -> (String, String, String) {
    let icon = format!("{}/images/flags/{}.png", HOMEPAGE, user.country);
    let url = format!("{}u/{}", HOMEPAGE, user.user_id);
    let text = format!(
        "{name}: {pp}pp (#{global} {country}{national})",
        name = user.username,
        pp = round_and_comma(user.pp_raw),
        global = with_comma_u64(user.pp_rank as u64),
        country = user.country,
        national = user.pp_country_rank
    );
    (icon, url, text)
}
