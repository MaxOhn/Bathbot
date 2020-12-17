use crate::{
    embeds::{osu, Author, EmbedData},
    util::{constants::AVATAR_URL, numbers::with_comma_u64},
};

use rosu::model::User;
use twilight_embed_builder::image_source::ImageSource;

pub struct RankRankedScoreEmbed {
    description: Option<String>,
    title: Option<String>,
    thumbnail: Option<ImageSource>,
    author: Option<Author>,
}

impl RankRankedScoreEmbed {
    pub fn new(user: User, rank: usize, rank_holder: User) -> Self {
        let title = format!(
            "How much ranked score is {name} missing to reach rank #{rank}?",
            name = user.username,
            rank = rank
        );
        let description = if user.ranked_score > rank_holder.ranked_score {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                ranked score**, so {name} is already above that with **{score} ranked score**.",
                rank = rank,
                holder_name = rank_holder.username,
                holder_score = with_comma_u64(rank_holder.ranked_score),
                name = user.username,
                score = with_comma_u64(user.ranked_score)
            )
        } else {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                 ranked score**, so {name} is missing **{missing}** score.",
                rank = rank,
                holder_name = rank_holder.username,
                holder_score = with_comma_u64(rank_holder.ranked_score),
                name = user.username,
                missing = with_comma_u64(rank_holder.ranked_score - user.ranked_score),
            )
        };
        Self {
            title: Some(title),
            description: Some(description),
            author: Some(osu::get_user_author(&user)),
            thumbnail: Some(ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap()),
        }
    }
}

impl EmbedData for RankRankedScoreEmbed {
    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }
    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }
    fn author_owned(&mut self) -> Option<Author> {
        self.author.take()
    }
    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }
}
