use crate::{
    backend::requests::Request,
    models::{GameMode, Score},
};

pub struct ScoresReq {
    pub user_id: Option<u16>,
    pub username: Option<String>,
    pub map_id: Option<u16>,
    pub mode: Option<GameMode>,
    pub mods: Option<u32>,
    pub limit: Option<u32>,
}

impl Request for ScoresReq {
    type Output = Vec<Score>;
    fn queue(&self) -> Self::Output {
        Vec::new()
    }
}

impl ScoresReq {
    pub fn new() -> Self {
        Self {
            user_id: None,
            username: None,
            map_id: None,
            mode: None,
            mods: None,
            limit: None,
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
