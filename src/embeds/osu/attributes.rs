use std::fmt::Write;

use command_macros::EmbedData;
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::GameMods;
use twilight_model::channel::embed::EmbedField;

use crate::util::{numbers::round, osu::AttributeKind};

#[derive(EmbedData)]
pub struct AttributesEmbed {
    fields: Vec<EmbedField>,
    title: String,
}

impl AttributesEmbed {
    pub fn new(kind: AttributeKind, value: f32, mods: GameMods) -> Self {
        let mut builder = BeatmapAttributesBuilder::default();

        match kind {
            AttributeKind::Ar => builder.ar(value),
            AttributeKind::Cs => builder.cs(value),
            AttributeKind::Hp => builder.hp(value),
            AttributeKind::Od => builder.od(value),
        };

        builder.mods(mods.bits());

        let title = format!(
            "Adjusting {}",
            match kind {
                AttributeKind::Ar => "AR",
                AttributeKind::Cs => "CS",
                AttributeKind::Hp => "HP",
                AttributeKind::Od => "OD",
            }
        );

        let nm_field = EmbedField {
            inline: true,
            name: "NM".to_owned(),
            value: round(value).to_string(),
        };

        let attrs = builder.build();

        let adjusted = match kind {
            AttributeKind::Ar => attrs.ar,
            AttributeKind::Cs => attrs.cs,
            AttributeKind::Hp => attrs.hp,
            AttributeKind::Od => attrs.od,
        };

        let mut mods_field = EmbedField {
            inline: true,
            name: mods.to_string(),
            value: round(adjusted as f32).to_string(),
        };

        if let AttributeKind::Ar = kind {
            let ms = attrs.hit_windows.ar;
            let _ = write!(mods_field.value, " ({}ms)", round(ms as f32));
        }

        let fields = vec![nm_field, mods_field];

        Self { title, fields }
    }
}
