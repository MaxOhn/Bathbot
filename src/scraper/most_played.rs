use rosu::models::GameMode;
use serde::{de, Deserialize, Deserializer};
use serde_derive::Deserialize as DeserializeDerive;

#[derive(Debug)]
pub struct MostPlayedMap {
    pub beatmap_id: u32,
    pub count: u32,
    pub mode: GameMode,
    pub title: String,
    pub artist: String,
    pub version: String,
    pub stars: f32,
}

impl<'de> Deserialize<'de> for MostPlayedMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(DeserializeDerive)]
        struct Outer {
            beatmap_id: u32,
            count: u32,
            beatmap: InnerMap,
            beatmapset: InnerMapset,
        }

        #[derive(DeserializeDerive)]
        pub struct InnerMap {
            #[serde(deserialize_with = "adjust_mode")]
            mode: GameMode,
            version: String,
            difficulty_rating: f32,
        }

        #[derive(DeserializeDerive)]
        pub struct InnerMapset {
            title: String,
            artist: String,
        }

        let helper = Outer::deserialize(deserializer)?;
        Ok(MostPlayedMap {
            beatmap_id: helper.beatmap_id,
            count: helper.count,
            mode: helper.beatmap.mode,
            title: helper.beatmapset.title,
            artist: helper.beatmapset.artist,
            version: helper.beatmap.version,
            stars: helper.beatmap.difficulty_rating,
        })
    }
}

fn adjust_mode<'de, D>(d: D) -> Result<GameMode, D::Error>
where
    D: Deserializer<'de>,
{
    let m: &str = Deserialize::deserialize(d)?;
    let m = match m {
        "osu" => GameMode::STD,
        "taiko" => GameMode::TKO,
        "fruits" => GameMode::CTB,
        "mania" => GameMode::MNA,
        _ => panic!("Could not parse mode '{}'", m),
    };
    Ok(m)
}
