use crate::{
    backend::requests::Request,
    models::{Beatmap, GameMode},
};

pub struct MapsReq {
    pub user_id: Option<u16>,
    pub username: Option<String>,
    pub map_id: Option<u16>,
    pub mapset_id: Option<u16>,
    pub mode: Option<GameMode>,
    pub limit: Option<u32>,
    pub mods: Option<u32>,
}

impl Request for MapsReq {
    type Output = Vec<Beatmap>;
    fn queue(&self) -> Self::Output {
        Vec::new()
    }
}

impl MapsReq {
    pub fn new() -> Self {
        Self {
            user_id: None,
            username: None,
            map_id: None,
            mapset_id: None,
            mode: None,
            limit: None,
            mods: None,
        }
    }

    pub fn user_id<'a>(&'a mut self, id: u16) -> &'a mut Self {
        self.user_id = Some(id);
        self
    }

    pub fn username<'a>(&'a mut self, name: String) -> &'a mut Self {
        self.username = Some(name);
        self
    }
}
