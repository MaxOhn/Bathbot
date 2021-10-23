use crate::{
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
    pub fn new(user: User, scores: Vec<Score>, pp: f32, rank: Option<usize>) -> Self {
        let stats = user.statistics.as_ref().unwrap();

        let title = format!(
            "What score is {name} missing to reach {pp_given}pp?",
            name = user.username,
            pp_given = with_comma_float(pp),
        );

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
            let (required, idx) = pp_missing(stats.pp, pp, &scores);

            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{required}pp** score which would be the top #{idx}",
                pp = with_comma_float(pp),
                user = user.username,
                required = with_comma_float(required),
                idx = idx
            )
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
