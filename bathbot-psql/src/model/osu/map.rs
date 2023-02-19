use rkyv::{Archive, Deserialize, Serialize};
use rosu_pp::{
    catch::CatchDifficultyAttributes, mania::ManiaDifficultyAttributes,
    osu::OsuDifficultyAttributes, taiko::TaikoDifficultyAttributes,
};

#[derive(Clone)]
pub struct DbBeatmap {
    pub map_id: i32,
    pub mapset_id: i32,
    pub user_id: i32,
    pub map_version: String,
    pub seconds_drain: i32,
    pub count_circles: i32,
    pub count_sliders: i32,
    pub count_spinners: i32,
    pub bpm: f32,
}

#[derive(Debug)]
pub enum DbMapFilename {
    Present(String),
    ChecksumMismatch,
    Missing,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct MapVersion {
    pub map_id: i32,
    pub version: String,
}

macro_rules! attr_struct {
    (
        $from:ident => $to:ident {
            $( $field:ident: $ty:ty $( as $to_ty:ty )?, )*
        }
    ) => {
        pub(crate) struct $from {
            $( pub(crate) $field: $ty, )*
        }

        impl From<$from> for $to {
            #[inline]
            fn from(attrs: $from) -> Self {
                let $from {
                    $( $field, )*
                } = attrs;

                #[allow(clippy::redundant_field_names)]
                Self {
                    $( $field: $field $( as $to_ty )?, )*
                }
            }
        }
    };
}

attr_struct!(DbOsuDifficultyAttributes => OsuDifficultyAttributes {
    aim: f64,
    speed: f64,
    flashlight: f64,
    slider_factor: f64,
    speed_note_count: f64,
    ar: f64,
    od: f64,
    hp: f64,
    n_circles: i32 as usize,
    n_sliders: i32 as usize,
    n_spinners: i32 as usize,
    stars: f64,
    max_combo: i32 as usize,
});

attr_struct!(DbTaikoDifficultyAttributes => TaikoDifficultyAttributes {
    stamina: f64,
    rhythm: f64,
    colour: f64,
    peak: f64,
    hit_window: f64,
    stars: f64,
    max_combo: i32 as usize,
});

attr_struct!(DbCatchDifficultyAttributes => CatchDifficultyAttributes {
    stars: f64,
    ar: f64,
    n_fruits: i32 as usize,
    n_droplets: i32 as usize,
    n_tiny_droplets: i32 as usize,
});

attr_struct!(DbManiaDifficultyAttributes => ManiaDifficultyAttributes {
    stars: f64,
    hit_window: f64,
    max_combo: i32 as usize,
});
