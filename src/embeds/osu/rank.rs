use crate::{
    embeds::{osu, Author, EmbedData},
    util::{constants::AVATAR_URL, numbers::with_comma, osu::pp_missing},
};

use rosu::models::{Score, User};
use std::borrow::Cow;
use twilight_embed_builder::image_source::ImageSource;

pub struct RankEmbed {
    description: String,
    title: String,
    thumbnail: ImageSource,
    author: Author,
}

impl RankEmbed {
    pub fn new(
        user: User,
        scores: Option<Vec<Score>>,
        rank: usize,
        country: Option<String>,
        rank_holder: User,
    ) -> Self {
        let country = country.map_or_else(|| Cow::Borrowed("#"), Cow::Owned);
        let title = format!(
            "How many pp is {name} missing to reach rank {country}{rank}?",
            name = user.username,
            country = country,
            rank = rank
        );
        let description = if user.pp_raw > rank_holder.pp_raw {
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is already above that with **{pp}pp**.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = with_comma(rank_holder.pp_raw),
                name = user.username,
                pp = with_comma(user.pp_raw)
            )
        } else if let Some(scores) = scores {
            let (required, _) = pp_missing(user.pp_raw, rank_holder.pp_raw, &scores);
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is missing **{missing}** raw pp, \
                 achievable with a single score worth **{pp}pp**.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = with_comma(rank_holder.pp_raw),
                name = user.username,
                missing = with_comma(rank_holder.pp_raw - user.pp_raw),
                pp = with_comma(required),
            )
        } else {
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is missing **{holder_pp}** raw pp, \
                 achievable with a single score worth **{holder_pp}pp**.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = with_comma(rank_holder.pp_raw),
                name = user.username,
            )
        };
        Self {
            title,
            description,
            author: osu::get_user_author(&user),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for RankEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
}
