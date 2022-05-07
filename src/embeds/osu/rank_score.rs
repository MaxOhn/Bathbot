use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::model::user::{User, UserCompact};

use crate::{
    custom_client::RespektiveUser,
    util::{
        builder::AuthorBuilder, constants::OSU_BASE, numbers::with_comma_int, osu::flag_url,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct RankRankedScoreEmbed {
    description: String,
    title: String,
    thumbnail: String,
    author: AuthorBuilder,
}

impl RankRankedScoreEmbed {
    pub fn new(
        user: User,
        rank: usize,
        rank_holder: UserCompact,
        respektive_user: Option<RespektiveUser>,
    ) -> Self {
        let user_score = user.statistics.as_ref().unwrap().ranked_score;
        let rank_holder_score = rank_holder.statistics.as_ref().unwrap().ranked_score;

        let title = format!(
            "How much ranked score is {name} missing to reach rank #{rank}?",
            name = user.username.cow_escape_markdown(),
        );

        let description = if user_score > rank_holder_score {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                ranked score**, so {name} is already above that with **{score} ranked score**.",
                holder_name = rank_holder.username.cow_escape_markdown(),
                holder_score = with_comma_int(rank_holder_score),
                name = user.username.cow_escape_markdown(),
                score = with_comma_int(user_score)
            )
        } else {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                 ranked score**, so {name} is missing **{missing}** score.",
                holder_name = rank_holder.username.cow_escape_markdown(),
                holder_score = with_comma_int(rank_holder_score),
                name = user.username.cow_escape_markdown(),
                missing = with_comma_int(rank_holder_score - user_score),
            )
        };

        let author = {
            let (ranked_score, rank) = match respektive_user {
                Some(user) => (user.ranked_score, Some(user.rank)),
                None => (user.statistics.unwrap().ranked_score, None),
            };

            let mut text = format!(
                "{name}: {score}",
                name = user.username.cow_escape_markdown(),
                score = with_comma_int(ranked_score),
            );

            if let Some(rank) = rank {
                let _ = write!(text, " (#{})", with_comma_int(rank));
            }

            let url = format!("{OSU_BASE}users/{}/{}", user.user_id, user.mode);
            let icon = flag_url(user.country_code.as_str());

            AuthorBuilder::new(text).url(url).icon_url(icon)
        };

        Self {
            author,
            description,
            title,
            thumbnail: user.avatar_url,
        }
    }
}
