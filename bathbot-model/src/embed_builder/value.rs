use serde::{Deserialize, Serialize};

use crate::deser::bool_as_u8;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SettingValue {
    #[serde(rename = "i")]
    pub inner: Value,
    pub y: u8,
}

impl SettingValue {
    pub const FOOTER_Y: u8 = u8::MAX;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Value {
    Grade,
    Mods,
    Score,
    #[serde(rename = "acc")]
    Accuracy,
    ScoreDate,
    Pp(PpValue),
    Combo(ComboValue),
    Hitresults(HitresultsValue),
    #[serde(rename = "len")]
    Length,
    Ar,
    Cs,
    Hp,
    Od,
    Bpm(EmoteTextValue),
    #[serde(rename = "n_obj")]
    CountObjects(EmoteTextValue),
    #[serde(rename = "n_slid")]
    CountSliders(EmoteTextValue),
    #[serde(rename = "n_spin")]
    CountSpinners(EmoteTextValue),
    #[serde(rename = "ranked_date")]
    MapRankedDate,
    Mapper(MapperValue),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PpValue {
    #[serde(
        default = "PpValue::default_max",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub max: bool,
    #[serde(
        default = "PpValue::default_if_fc",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub if_fc: bool,
}

impl PpValue {
    fn default_max() -> bool {
        true
    }

    fn default_if_fc() -> bool {
        true
    }
}

impl Default for PpValue {
    fn default() -> Self {
        Self {
            max: Self::default_max(),
            if_fc: Self::default_if_fc(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ComboValue {
    #[serde(
        default = "ComboValue::default_max",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub max: bool,
}

impl ComboValue {
    fn default_max() -> bool {
        true
    }
}

impl Default for ComboValue {
    fn default() -> Self {
        Self {
            max: Self::default_max(),
        }
    }
}

define_enum! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum HitresultsValue {
        Full = 0,
        #[default]
        OnlyMisses = 1,
    }
}

define_enum! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum EmoteTextValue {
        #[default]
        Emote = 0,
        Text = 1,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct MapperValue {
    #[serde(
        rename = "status",
        default = "MapperValue::default_with_status",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub with_status: bool,
}

impl MapperValue {
    fn default_with_status() -> bool {
        true
    }
}

impl Default for MapperValue {
    fn default() -> Self {
        Self {
            with_status: Self::default_with_status(),
        }
    }
}
