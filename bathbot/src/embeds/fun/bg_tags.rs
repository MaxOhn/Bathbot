use bathbot_macros::EmbedData;
use bathbot_model::{Effects, MapsetTags};
use bathbot_util::FooterBuilder;
use twilight_model::channel::message::embed::EmbedField;

use crate::commands::fun::GameDifficulty;

#[derive(EmbedData)]
pub struct BGTagsEmbed {
    description: &'static str,
    fields: Vec<EmbedField>,
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

        let fields = fields![
            "Included", include_value, true;
            "Excluded", excluded_value, true;
            "Effects", effects_value, true;
        ];

        let description = (amount == 0)
            .then_some("No stored backgrounds match these tags, try different ones")
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
