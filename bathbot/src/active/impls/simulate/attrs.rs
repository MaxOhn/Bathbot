use rosu_pp::Beatmap;

#[derive(Copy, Clone, Default)]
pub struct SimulateAttributes {
    pub ar: Option<f32>,
    pub cs: Option<f32>,
    pub hp: Option<f32>,
    pub od: Option<f32>,
}

impl From<&Beatmap> for SimulateAttributes {
    #[inline]
    fn from(map: &Beatmap) -> Self {
        Self {
            ar: Some(map.ar),
            cs: Some(map.cs),
            hp: Some(map.hp),
            od: Some(map.od),
        }
    }
}
