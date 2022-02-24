use crate::embeds::{Author, Footer};

pub struct MedalsListEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl MedalsListEmbed {
    pub fn new() -> Self {
        todo!()
    }
}

impl_builder!(MedalsListEmbed {
    author,
    description,
    footer,
    thumbnail,
});
