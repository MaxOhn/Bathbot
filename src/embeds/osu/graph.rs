use rosu_v2::prelude::User;

use crate::{embeds::{attachment, }, util::builder::AuthorBuilder};

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

impl_builder!(GraphEmbed { author, image });
