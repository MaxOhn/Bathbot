use twilight_model::id::{marker::UserMarker, Id};

pub struct BGStartEmbed {
    description: String,
}

impl BGStartEmbed {
    pub fn new(author: Id<UserMarker>) -> Self {
        let description = format!(
            "**React to include tag, unreact to exclude tag.**\n\
            <@{author}> react with ✅ when you're ready.\n\
            ```\n\
            🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
            🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Lock in\n\
            🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old     ❌: Abort\n\
            ```"
        );

        Self { description }
    }
}

impl_builder!(BGStartEmbed { description });
