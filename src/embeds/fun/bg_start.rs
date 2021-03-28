use twilight_model::id::UserId;

pub struct BGStartEmbed {
    description: String,
}

impl BGStartEmbed {
    pub fn new(author: UserId) -> Self {
        let description = format!(
            "**React to include tag, unreact to exclude tag.**\n\
            <@{}> react with ✅ when you're ready.\n\
            ```\n\
            🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
            🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Lock in\n\
            🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old     ❌: Abort\n\
            ```",
            author
        );

        Self { description }
    }
}

impl_into_builder!(BGStartEmbed { description });
