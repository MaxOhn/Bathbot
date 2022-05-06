use std::fmt::Write;

use command_macros::EmbedData;
use rosu_pp::Mods;
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
        let adjusted = kind.apply(value, mods);

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

        let mut mods_field = EmbedField {
            inline: true,
            name: mods.to_string(),
            value: round(adjusted).to_string(),
        };

        if let AttributeKind::Ar = kind {
            let mods = mods.bits();
            let value = value * mods.od_ar_hp_multiplier() as f32;
            let ms = AttributeKind::ar_ms(value, mods.clock_rate() as f32);

            let _ = write!(mods_field.value, " ({ms}ms)");
        }

        let fields = vec![nm_field, mods_field];

        Self { title, fields }
    }
}
