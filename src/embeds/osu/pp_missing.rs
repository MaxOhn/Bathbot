use crate::{
    embeds::{osu, Author, EmbedData},
    util::{constants::AVATAR_URL, numbers::round_and_comma, osu::pp_missing},
};

use rosu::models::{Score, User};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct PPMissingEmbed {
    description: String,
    title: String,
    thumbnail: ImageSource,
    author: Author,
}

impl PPMissingEmbed {
    pub fn new(user: User, scores: Vec<Score>, pp: f32) -> Self {
        let title = format!(
            "What score is {name} missing to reach {pp_given:.2}pp?",
            name = user.username,
            pp_given = pp
        );
        let description = if scores.is_empty() {
            format!(
                "To reach {pp:.2}pp with one additional score, {user} needs to perform \
                 a **{pp:.2}pp** score which would be the top #1",
                pp = pp,
                user = user.username,
            )
        } else if user.pp_raw > pp {
            format!(
                "{name} already has {pp_raw}pp which is more than {pp_given:.2}pp.\n\
                 No more scores are required.",
                name = user.username,
                pp_raw = round_and_comma(user.pp_raw),
                pp_given = pp
            )
        } else {
            let (required, idx) = pp_missing(user.pp_raw, pp, &scores);
            format!(
                "To reach {pp:.2}pp with one additional score, {user} needs to perform \
                 a **{required:.2}pp** score which would be the top #{idx}",
                pp = pp,
                user = user.username,
                required = required,
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
