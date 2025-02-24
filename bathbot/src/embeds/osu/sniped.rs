use bathbot_macros::EmbedData;
use bathbot_model::SnipedWeek;
use bathbot_util::{AuthorBuilder, fields};
use twilight_model::channel::message::embed::EmbedField;

use crate::{embeds::attachment, manager::redis::osu::CachedUser, util::CachedUserExt};

#[derive(EmbedData)]
pub struct SnipedEmbed {
    author: AuthorBuilder,
    description: String,
    fields: Vec<EmbedField>,
    image: String,
    thumbnail: String,
    title: &'static str,
}

impl SnipedEmbed {
    pub fn new(user: &CachedUser, sniper: Vec<SnipedWeek>, snipee: Vec<SnipedWeek>) -> Self {
        let thumbnail = user.avatar_url.as_ref().to_owned();
        let author = user.author_builder(false);
        let title = "National snipe scores of the last 8 weeks";
        let username = user.username.as_str();

        if sniper.is_empty() && snipee.is_empty() {
            let description =
                format!("`{username}` neither sniped others nor was sniped by others");

            return Self {
                author,
                description,
                fields: Vec::new(),
                image: String::new(),
                thumbnail,
                title,
            };
        }

        let mut fields = Vec::with_capacity(2);

        if !sniper.is_empty() {
            let last_week = &sniper[0];
            let most_player = &last_week.players[0];
            let name = format!("Sniped by {username}:");

            let value = format!(
                "Total count: {total}\n\
                Different victims: {unique}\n\
                Targeted the most: {most_name} ({most_count})",
                total = sniper[0].total,
                unique = sniper[0].unique,
                most_name = most_player.username,
                // The count values were accumulated for the graph so we need
                // to de-accumulate again
                most_count =
                    most_player.count - last_week.players.get(1).map_or(0, |player| player.count),
            );

            fields![fields { name, value, false }];
        }

        if !snipee.is_empty() {
            let last_week = &snipee[0];
            let most_player = &last_week.players[0];
            let name = format!("Sniped {username}");

            let value = format!(
                "Total count: {total}\n\
                Different snipers: {unique}\n\
                Targeted the most: {most_name} ({most_count})",
                total = snipee[0].total,
                unique = snipee[0].unique,
                most_name = most_player.username,
                // The count values were accumulated for the graph so we need
                // to de-accumulate again
                most_count =
                    most_player.count - last_week.players.get(1).map_or(0, |player| player.count),
            );

            fields![fields { name, value, false }];
        }

        Self {
            author,
            description: String::new(),
            fields,
            image: attachment("sniped_graph.png"),
            thumbnail,
            title,
        }
    }
}
