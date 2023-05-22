use std::collections::HashMap;

use bathbot_macros::EmbedData;
use bathbot_model::{rosu_v2::user::User, SnipeRecent};
use bathbot_util::{fields, AuthorBuilder};
use twilight_model::channel::message::embed::EmbedField;

use crate::{embeds::attachment, manager::redis::RedisData};

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
            let mut most_count = 0;
            let mut most_name = None;

            for score in snipee.iter() {
                let entry = snipers.entry(score.sniper_id).or_insert(0);
                *entry += 1;

                if *entry > most_count {
                    most_count = *entry;

                    if let Some(sniper) = score.sniper.as_deref() {
                        most_name = Some(sniper);
                    }
                }
            }

            // should technically always be `Some` but huismetbenen is bugged
            let most_name = most_name.unwrap_or("<unknown user>");
            let name = format!("Sniped {username}");

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
