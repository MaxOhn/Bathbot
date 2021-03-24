use crate::{
    custom_client::SnipeRecent,
    embeds::{Author, EmbedData, EmbedFields},
    util::constants::AVATAR_URL,
};

use rosu_v2::model::user::User;
use std::collections::HashMap;
use twilight_embed_builder::image_source::ImageSource;

pub struct SnipedEmbed {
    description: Option<String>,
    thumbnail: Option<ImageSource>,
    title: &'static str,
    author: Option<Author>,
    image: Option<ImageSource>,
    fields: Option<EmbedFields>,
}

impl SnipedEmbed {
    pub fn new(user: User, sniper: Vec<SnipeRecent>, snipee: Vec<SnipeRecent>) -> Self {
        let thumbnail = ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap();
        let author = author!(user);
        let title = "National snipe scores of the last 8 weeks";

        if sniper.is_empty() && snipee.is_empty() {
            let description = format!(
                "`{}` was neither sniped nor sniped other people",
                user.username
            );
            return Self {
                description: Some(description),
                thumbnail: Some(thumbnail),
                author: Some(author),
                title,
                image: None,
                fields: None,
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

            fields.push((name, value, false));
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

            fields.push((name, value, false));
        }

        Self {
            title,
            author: Some(author),
            thumbnail: Some(thumbnail),
            description: None,
            fields: Some(fields),
            image: Some(ImageSource::attachment("sniped_graph.png").unwrap()),
        }
    }
}

impl EmbedData for SnipedEmbed {
    fn title_owned(&mut self) -> Option<String> {
        Some(self.title.to_owned())
    }

    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }

    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }

    fn image_owned(&mut self) -> Option<ImageSource> {
        self.image.take()
    }

    fn author_owned(&mut self) -> Option<Author> {
        self.author.take()
    }

    fn fields_owned(self) -> Option<EmbedFields> {
        self.fields
    }
}
