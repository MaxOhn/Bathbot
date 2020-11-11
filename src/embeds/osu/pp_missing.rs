use crate::{
    embeds::{osu, Author, EmbedData},
    util::{constants::AVATAR_URL, numbers::with_comma, osu::pp_missing},
};

use rosu::model::{Score, User};
use twilight_embed_builder::image_source::ImageSource;

pub struct PPMissingEmbed {
    description: String,
    title: String,
    thumbnail: ImageSource,
    author: Author,
}

impl PPMissingEmbed {
    pub fn new(user: User, scores: Vec<Score>, pp: f32) -> Self {
        let title = format!(
            "What score is {name} missing to reach {pp_given}pp?",
            name = user.username,
            pp_given = with_comma(pp)
        );
        let description = if scores.is_empty() {
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{pp}pp** score which would be the top #1",
                pp = with_comma(pp),
                user = user.username,
            )
        } else if user.pp_raw > pp {
            format!(
                "{name} has {pp_raw}pp which is already more than {pp_given}pp.",
                name = user.username,
                pp_raw = with_comma(user.pp_raw),
                pp_given = with_comma(pp)
            )
        } else {
            let (required, idx) = pp_missing(user.pp_raw, pp, &scores);
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{required}pp** score which would be the top #{idx}",
                pp = with_comma(pp),
                user = user.username,
                required = with_comma(required),
                idx = idx
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

impl EmbedData for PPMissingEmbed {
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
