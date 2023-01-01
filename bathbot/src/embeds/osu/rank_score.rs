use std::fmt::Write;

use bathbot_macros::EmbedData;
use rosu_v2::model::user::UserCompact;

use crate::{
    custom_client::RespektiveUser,
    manager::redis::{osu::User, RedisData},
    util::{
        builder::AuthorBuilder, constants::OSU_BASE, numbers::WithComma, osu::flag_url, CowUtils,
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
        user: &RedisData<User>,
        rank: usize,
        rank_holder: UserCompact,
        respektive_user: Option<RespektiveUser>,
    ) -> Self {
        let user_score = user.peek_stats(|stats| stats.ranked_score);
        let username = user.username().cow_escape_markdown();
        let rank_holder_score = rank_holder.statistics.as_ref().unwrap().ranked_score;

        let title = format!("How much ranked score is {username} missing to reach rank #{rank}?");

        let description = if user_score > rank_holder_score {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                ranked score**, so {username} is already above that with **{score} ranked score**.",
                holder_name = rank_holder.username.cow_escape_markdown(),
                holder_score = WithComma::new(rank_holder_score),
                score = WithComma::new(user_score)
            )
        } else {
            format!(
                "Rank #{rank} is currently held by {holder_name} with **{holder_score} \
                 ranked score**, so {username} is missing **{missing}** score.",
                holder_name = rank_holder.username.cow_escape_markdown(),
                holder_score = WithComma::new(rank_holder_score),
                missing = WithComma::new(rank_holder_score - user_score),
            )
        };

        let author = {
            let (ranked_score, rank) = match respektive_user {
                Some(user) => (user.ranked_score, Some(user.rank)),
                None => (user.peek_stats(|stats| stats.ranked_score), None),
            };

            let mut text = format!("{username}: {score}", score = WithComma::new(ranked_score),);

            if let Some(rank) = rank {
                let _ = write!(text, " (#{})", WithComma::new(rank));
            }

            let (country_code, user_id, mode) = match user {
                RedisData::Original(user) => {
                    let country_code = user.country_code.as_str();
                    let user_id = user.user_id;
                    let mode = user.mode;

                    (country_code, user_id, mode)
                }
                RedisData::Archived(user) => {
                    let country_code = user.country_code.as_str();
                    let user_id = user.user_id;
                    let mode = user.mode;

                    (country_code, user_id, mode)
                }
            };

            let url = format!("{OSU_BASE}users/{user_id}/{mode}");
            let icon = flag_url(country_code);

            AuthorBuilder::new(text).url(url).icon_url(icon)
        };

        Self {
            author,
            description,
            title,
            thumbnail: user.avatar_url().to_owned(),
        }
    }
}
