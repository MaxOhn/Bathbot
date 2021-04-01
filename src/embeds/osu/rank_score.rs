use crate::{
    embeds::Author,
    util::{constants::AVATAR_URL, numbers::with_comma_uint},
};

use rosu_v2::model::user::{User, UserCompact};

pub struct RankRankedScoreEmbed {
    description: String,
    title: String,
    thumbnail: String,
    author: Author,
}

impl RankRankedScoreEmbed {
    pub fn new(user: User, rank: usize, rank_holder: UserCompact) -> Self {
        let user_score = user.statistics.as_ref().unwrap().ranked_score;
        let rank_holder_score = rank_holder.statistics.as_ref().unwrap().ranked_score;

        let title = format!(
            "How much ranked score is {name} missing to reach rank #{rank}?",
            name = user.username,
            rank = rank
        );

        let description = if user_score > rank_holder_score {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                ranked score**, so {name} is already above that with **{score} ranked score**.",
                rank = rank,
                holder_name = rank_holder.username,
                holder_score = with_comma_uint(rank_holder_score),
                name = user.username,
                score = with_comma_uint(user_score)
            )
        } else {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                 ranked score**, so {name} is missing **{missing}** score.",
                rank = rank,
                holder_name = rank_holder.username,
                holder_score = with_comma_uint(rank_holder_score),
                name = user.username,
                missing = with_comma_uint(rank_holder_score - user_score),
            )
        };

        Self {
            title,
            description,
            author: author!(user),
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
        }
    }
}

impl_builder!(RankRankedScoreEmbed {
    author,
    description,
    thumbnail,
    title,
});
