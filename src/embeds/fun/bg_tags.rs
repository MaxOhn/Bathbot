use crate::{bg_game::MapsetTags, embeds::EmbedFields};

pub struct BGTagsEmbed {
    description: &'static str,
    fields: EmbedFields,
    title: &'static str,
}

impl BGTagsEmbed {
    pub fn new(included: MapsetTags, excluded: MapsetTags, amount: usize) -> Self {
        let include_value = if !included.is_empty() {
            included.join("\n")
        } else if excluded.is_empty() {
            "Any".to_owned()
        } else {
            "None".to_owned()
        };

        let excluded_value = if !excluded.is_empty() {
            excluded.join("\n")
        } else {
            "None".to_owned()
        };

        let fields = vec![
            field!("Included", include_value, true),
            field!("Excluded", excluded_value, true),
            field!("#Backgrounds", amount.to_string(), true),
        ];

        let description = (amount == 0)
            .then(|| "No stored backgrounds match these tags, try different ones")
            .unwrap_or_default();

        Self {
            fields,
            description,
            title: "Selected tags",
        }
    }
}

impl_builder!(BGTagsEmbed {
    description,
    fields,
    title,
});
