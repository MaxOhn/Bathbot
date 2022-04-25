use command_macros::EmbedData;

use crate::{
    commands::fun::GameDifficulty,
    embeds::EmbedFields,
    games::bg::{Effects, MapsetTags},
    util::builder::FooterBuilder,
};

#[derive(EmbedData)]
pub struct BGTagsEmbed {
    description: &'static str,
    fields: EmbedFields,
    footer: FooterBuilder,
    title: String,
}

impl BGTagsEmbed {
    pub fn new(
        included: MapsetTags,
        excluded: MapsetTags,
        amount: usize,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> Self {
        let include_value = if !included.is_empty() {
            included.join('\n')
        } else if excluded.is_empty() {
            "Any".to_owned()
        } else {
            "None".to_owned()
        };

        let excluded_value = if !excluded.is_empty() {
            excluded.join('\n')
        } else {
            "None".to_owned()
        };

        let effects_value = if !effects.is_empty() {
            effects.join('\n')
        } else {
            "None".to_owned()
        };

        let fields = vec![
            field!("Included", include_value, true),
            field!("Excluded", excluded_value, true),
            field!("Effects", effects_value, true),
        ];

        let description = (amount == 0)
            .then(|| "No stored backgrounds match these tags, try different ones")
            .unwrap_or_default();

        let footer = FooterBuilder::new(format!("Difficulty: {difficulty:?}"));

        Self {
            description,
            fields,
            footer,
            title: format!("Selected tags ({amount} backgrounds)"),
        }
    }
}