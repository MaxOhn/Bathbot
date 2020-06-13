use crate::util::{
    globals::{AVATAR_URL, HOMEPAGE},
    numbers::{round, round_and_comma, with_comma_u64},
};

use rosu::models::{GameMode, Score, User};
use serenity::{builder::CreateEmbed, utils::Colour};
use std::f32;

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
            let (required, idx) = osu::pp_missing(user.pp_raw, pp, &scores);
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
            let (required, _) = osu::pp_missing(user.pp_raw, rank_holder.pp_raw, &scores);
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
