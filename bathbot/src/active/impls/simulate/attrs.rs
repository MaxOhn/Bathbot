use crate::manager::OsuMap;

#[derive(Copy, Clone, Default)]
pub struct SimulateAttributes {
    pub ar: Option<f32>,
    pub cs: Option<f32>,
    pub hp: Option<f32>,
    pub od: Option<f32>,
}

impl From<&OsuMap> for SimulateAttributes {
    #[inline]
    fn from(map: &OsuMap) -> Self {
        Self {
            ar: Some(map.ar()),
            cs: Some(map.cs()),
            hp: Some(map.hp()),
            od: Some(map.od()),
        }
    }
}
