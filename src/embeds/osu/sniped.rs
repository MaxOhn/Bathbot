use crate::{
    custom_client::SnipeRecent,
    embeds::{attachment, Author, EmbedFields},
    util::constants::AVATAR_URL,
};

use rosu_v2::model::user::User;
use std::collections::HashMap;

pub struct SnipedEmbed {
    author: Author,
    description: String,
    fields: EmbedFields,
    image: String,
    thumbnail: String,
    title: &'static str,
}

impl SnipedEmbed {
    pub fn new(user: User, sniper: Vec<SnipeRecent>, snipee: Vec<SnipeRecent>) -> Self {
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let author = author!(user);
        let title = "National snipe scores of the last 8 weeks";

        if sniper.is_empty() && snipee.is_empty() {
            let description = format!(
                "`{}` was neither sniped nor sniped other people",
                user.username
            );

            return Self {
                author,
                description,
                fields: Vec::new(),
                image: String::new(),
                thumbnail,
                title,
            };
        }

        let mut fields = EmbedFields::with_capacity(2);

        if !sniper.is_empty() {
            let mut victims = HashMap::new();

            for score in sniper.iter() {
                *victims.entry(score.sniped.as_deref().unwrap()).or_insert(0) += 1;
            }

            let (most_name, most_count) = victims.iter().max_by_key(|(_, count)| *count).unwrap();
            let name = format!("Sniped by {}:", user.username);

            let value = format!(
                "Total count: {}\n\
                Different victims: {}\n\
                Targeted the most: {} ({})",
                sniper.len(),
                victims.len(),
                most_name,
                most_count
            );

            fields.push(field!(name, value, false));
        }

        if !snipee.is_empty() {
            let mut snipers = HashMap::new();

            for score in snipee.iter() {
                *snipers.entry(score.sniper.as_str()).or_insert(0) += 1;
            }

            let (most_name, most_count) = snipers.iter().max_by_key(|(_, count)| *count).unwrap();
            let name = format!("Sniped {}:", user.username);

            let value = format!(
                "Total count: {}\n\
                Different snipers: {}\n\
                Targeted the most: {} ({})",
                snipee.len(),
                snipers.len(),
                most_name,
                most_count
            );

            fields.push(field!(name, value, false));
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

impl_into_builder!(SnipedEmbed {
    author,
    description,
    fields,
    image,
    thumbnail,
    title,
});
