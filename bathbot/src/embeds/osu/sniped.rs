use std::collections::HashMap;

use bathbot_macros::EmbedData;
use twilight_model::channel::embed::EmbedField;

use crate::{
    custom_client::SnipeRecent,
    embeds::attachment,
    manager::redis::{osu::User, RedisData},
    util::builder::AuthorBuilder,
};

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
    pub fn new(user: &RedisData<User>, sniper: Vec<SnipeRecent>, snipee: Vec<SnipeRecent>) -> Self {
        let thumbnail = user.avatar_url().to_owned();
        let author = user.author_builder();
        let title = "National snipe scores of the last 8 weeks";
        let username = user.username();

        if sniper.is_empty() && snipee.is_empty() {
            let description = format!("`{username}` was neither sniped nor sniped other people");

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
            let mut victims = HashMap::new();

            for score in sniper.iter() {
                if let Some(name) = score.sniped.as_deref() {
                    *victims.entry(name).or_insert(0) += 1;
                }
            }

            let (most_name, most_count) = victims.iter().max_by_key(|(_, count)| *count).unwrap();
            let name = format!("Sniped by {username}:");

            let value = format!(
                "Total count: {}\n\
                Different victims: {}\n\
                Targeted the most: {most_name} ({most_count})",
                sniper.len(),
                victims.len(),
            );

            fields![fields { name, value, false }];
        }

        if !snipee.is_empty() {
            let mut snipers = HashMap::new();

            for score in snipee.iter() {
                *snipers.entry(score.sniper.as_str()).or_insert(0) += 1;
            }

            let (most_name, most_count) = snipers.iter().max_by_key(|(_, count)| *count).unwrap();
            let name = format!("Sniped {username}:");

            let value = format!(
                "Total count: {}\n\
                Different snipers: {}\n\
                Targeted the most: {most_name} ({most_count})",
                snipee.len(),
                snipers.len(),
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
