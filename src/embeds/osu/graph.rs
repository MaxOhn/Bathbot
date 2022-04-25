use command_macros::EmbedData;
use rosu_v2::prelude::User;

use crate::{embeds::attachment, util::builder::AuthorBuilder};

#[derive(EmbedData)]
pub struct GraphEmbed {
    author: AuthorBuilder,
    image: String,
}

impl GraphEmbed {
    pub fn new(user: &User) -> Self {
        Self {
            author: author!(user),
            image: attachment("graph.png"),
        }
    }
}
