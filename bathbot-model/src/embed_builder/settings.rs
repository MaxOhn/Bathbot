use serde::{Deserialize, Serialize};

use super::{SettingValue, Value};
use crate::deser::bool_as_u8;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ScoreEmbedSettings {
    #[serde(rename = "v")]
    pub values: Vec<SettingValue>,
    #[serde(
        rename = "a",
        default = "ScoreEmbedSettings::default_show_artist",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub show_artist: bool,
    #[serde(
        rename = "s",
        default = "ScoreEmbedSettings::default_show_sr_in_title",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub show_sr_in_title: bool,
    #[serde(rename = "i")]
    pub image: SettingsImage,
    #[serde(rename = "b")]
    pub buttons: SettingsButtons,
}

impl ScoreEmbedSettings {
    fn default_show_artist() -> bool {
        true
    }

    fn default_show_sr_in_title() -> bool {
        true
    }
}

impl Default for ScoreEmbedSettings {
    fn default() -> Self {
        Self {
            values: vec![
                SettingValue {
                    inner: Value::Grade,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Mods,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Score,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Accuracy,
                    y: 0,
                },
                SettingValue {
                    inner: Value::ScoreDate,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Pp(Default::default()),
                    y: 1,
                },
                SettingValue {
                    inner: Value::Combo(Default::default()),
                    y: 1,
                },
                SettingValue {
                    inner: Value::Hitresults(Default::default()),
                    y: 1,
                },
                SettingValue {
                    inner: Value::Length,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Cs,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Ar,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Od,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Hp,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Bpm(Default::default()),
                    y: 2,
                },
                SettingValue {
                    inner: Value::Mapper(Default::default()),
                    y: SettingValue::FOOTER_Y,
                },
                SettingValue {
                    inner: Value::MapRankedDate,
                    y: SettingValue::FOOTER_Y,
                },
            ],
            show_artist: Self::default_show_artist(),
            show_sr_in_title: Self::default_show_sr_in_title(),
            image: SettingsImage::default(),
            buttons: SettingsButtons::default(),
        }
    }
}

define_enum! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum SettingsImage {
        #[default]
        Thumbnail = 0,
        Image = 1,
        Hide = 2,
        ImageWithStrains = 3,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SettingsButtons {
    #[serde(
        default = "SettingsButtons::default_pagination",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub pagination: bool,
    #[serde(
        default = "SettingsButtons::default_render",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub render: bool,
    #[serde(
        default = "SettingsButtons::default_miss_analyzer",
        with = "bool_as_u8",
        skip_serializing_if = "super::is_true"
    )]
    pub miss_analyzer: bool,
}

impl SettingsButtons {
    fn default_pagination() -> bool {
        true
    }

    fn default_render() -> bool {
        true
    }

    fn default_miss_analyzer() -> bool {
        true
    }
}

impl Default for SettingsButtons {
    fn default() -> Self {
        Self {
            pagination: Self::default_pagination(),
            render: Self::default_render(),
            miss_analyzer: Self::default_miss_analyzer(),
        }
    }
}
