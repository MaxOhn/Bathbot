use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_util::{numbers::round, osu::AttributeKind};
use rosu_pp::model::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::GameModsIntermode;
use twilight_model::channel::message::embed::EmbedField;

#[derive(EmbedData)]
pub struct AttributesEmbed {
    fields: Vec<EmbedField>,
    title: String,
}

impl AttributesEmbed {
    pub fn new(
        kind: AttributeKind,
        value: f32,
        mods: GameModsIntermode,
        clock_rate: Option<f32>,
    ) -> Self {
        let mut builder = BeatmapAttributesBuilder::default().mods(&mods);

        builder = match kind {
            AttributeKind::Ar => builder.ar(value, false),
            AttributeKind::Cs => builder.cs(value, false),
            AttributeKind::Hp => builder.hp(value, false),
            AttributeKind::Od => builder.od(value, false),
        };

        if let Some(clock_rate) = clock_rate {
            builder = builder.clock_rate(clock_rate as f64);
        }

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

        let mut mods_name = mods.to_string();

        if let Some(clock_rate) = clock_rate.map(round) {
            let _ = write!(mods_name, "({clock_rate}x)");
        }

        let mut mods_field = EmbedField {
            inline: true,
            name: mods_name,
            value: round(adjusted as f32).to_string(),
        };

        let ms = match kind {
            AttributeKind::Ar => Some(attrs.hit_windows.ar),
            AttributeKind::Od => Some(attrs.hit_windows.od_great),
            AttributeKind::Cs | AttributeKind::Hp => None,
        };

        if let Some(ms) = ms {
            let _ = write!(mods_field.value, " ({}ms)", round(ms as f32));
        }

        let fields = vec![nm_field, mods_field];

        Self { title, fields }
    }
}
