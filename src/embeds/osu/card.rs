use command_macros::EmbedData;
use rosu_v2::prelude::User;

use crate::{embeds::attachment, util::builder::AuthorBuilder};

#[derive(EmbedData)]
pub struct CardEmbed {
    author: AuthorBuilder,
    image: String,
}

impl CardEmbed {
    pub fn new(user: &User) -> Self {
        Self {
            author: author!(user),
            image: attachment("card.png"),
        }
    }
}
