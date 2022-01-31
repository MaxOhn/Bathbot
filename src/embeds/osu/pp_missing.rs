use crate::{
    commands::osu::PpVersion,
    embeds::{Author, EmbedBuilder, EmbedData, Footer},
    util::{
        numbers::{with_comma_float, with_comma_int},
        osu::pp_missing,
    },
};

use rosu_v2::prelude::{Score, User};

pub struct PPMissingEmbed {
    author: Author,
    description: String,
    footer: Option<Footer>,
    thumbnail: String,
    title: String,
}

impl PPMissingEmbed {
    pub fn new(
        user: User,
        scores: &mut [Score],
        pp: f32,
        rank: Option<usize>,
        version: PpVersion,
    ) -> Self {
        let stats = user.statistics.as_ref().unwrap();

        let title = format!(
            "What scores is {name} missing to reach {pp_given}pp?",
            name = user.username,
            pp_given = with_comma_float(pp),
        );

        // Filling the top100 with scores each worth the same x pp:
        // Σ_i=0^99 (x * 0.95^i) = x * Σ_i=0^99 (0.95^i) = x * 19.881594
        const FACTOR: f32 = 19.881594;

        let description = if scores.is_empty() {
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{pp}pp** score which would be the top #1",
                pp = with_comma_float(pp),
                user = user.username,
            )
        } else if stats.pp > pp {
            format!(
                "{name} has {pp_raw}pp which is already more than {pp_given}pp.",
                name = user.username,
                pp_raw = with_comma_float(stats.pp),
                pp_given = with_comma_float(pp)
            )
        } else {
            let (required, idx) = pp_missing(stats.pp, pp, scores);
            let top_pp = scores[0].pp.unwrap_or(0.0);

            let bot: f32 = scores
                .iter()
                .skip(1)
                .filter_map(|s| s.weight)
                .map(|w| w.pp)
                .sum();

            let bonus_pp = stats.pp - (top_pp + bot);

            if required <= top_pp || version == PpVersion::Single {
                format!(
                    "To reach {pp}pp with one additional score, {user} needs to perform \
                    a **{required}pp** score which would be the top #{idx}",
                    pp = with_comma_float(pp),
                    user = user.username,
                    required = with_comma_float(required),
                )
            } else if top_pp * FACTOR + bonus_pp < pp {
                format!(
                    "If the entire top100 was filled with {top_pp}pp scores, \
                    the total would still only be **{max_pp}pp** which is less than {pp}pp.",
                    top_pp = with_comma_float(top_pp),
                    max_pp = with_comma_float(top_pp * FACTOR + bonus_pp),
                    pp = with_comma_float(pp),
                )
            } else {
                let mut top = top_pp + bonus_pp;
                let mut idx = 99;
                let len = scores.len();

                for i in 1..scores.len() {
                    let bot: f32 = scores
                        .iter_mut()
                        .skip(1)
                        .take(len - i - 1)
                        .filter_map(|s| s.weight.as_mut())
                        .map(|w| {
                            w.pp *= 0.95;

                            w.pp
                        })
                        .sum();

                    let factor = 0.95_f32.powi(i as i32);

                    if top + factor * top_pp + bot >= pp {
                        // requires idx many new scores of top_pp many pp and one additional score
                        idx = i - 1;
                        break;
                    }

                    top += factor * top_pp;
                }

                // Shift scores to right and then overwrite pp values with top_pp
                scores[1..].rotate_right(idx);

                scores
                    .iter_mut()
                    .skip(1)
                    .take(idx)
                    .for_each(|s| s.pp = Some(top_pp));

                let bot: f32 = scores
                    .iter()
                    .skip(idx + 1)
                    .take(len - idx - 1)
                    .filter_map(|s| s.weight.as_ref())
                    .map(|w| w.pp / 0.95)
                    .sum();

                if idx == 99 {
                    format!(
                        "To reach {pp}pp, {user} needs to perform 99 more **{top_pp}pp** scores",
                        top_pp = with_comma_float(top_pp),
                        pp = with_comma_float(pp),
                        user = user.username,
                    )
                } else {
                    // Calculate the pp of the missing score after adding idx many top_pp scores
                    let total = top + bot;
                    let (required, _) = pp_missing(total, pp, scores);

                    format!(
                        "To reach {pp}pp, {user} needs to perform {amount} more \
                        **{top_pp}pp** score{plural} and one **{required}pp** score.",
                        amount = idx,
                        top_pp = with_comma_float(top_pp),
                        plural = if idx != 1 { "s" } else { "" },
                        pp = with_comma_float(pp),
                        user = user.username,
                        required = with_comma_float(required),
                    )
                }
            }
        };

        let footer = rank.map(|rank| {
            Footer::new(format!(
                "The current rank for {pp}pp is #{rank}",
                pp = with_comma_float(pp),
                rank = with_comma_int(rank),
            ))
        });

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url,
            title,
        }
    }
}

impl EmbedData for PPMissingEmbed {
    fn into_builder(self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .thumbnail(self.thumbnail)
            .title(self.title);

        if let Some(footer) = self.footer {
            builder.footer(footer)
        } else {
            builder
        }
    }
}
