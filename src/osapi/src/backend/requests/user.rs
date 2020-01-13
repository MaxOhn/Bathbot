use crate::{
    backend::requests::Request,
    models::{GameMode, User},
};

pub struct UserReq {
    pub user_id: Option<u16>,
    pub username: Option<String>,
    pub mode: Option<GameMode>,
}

impl Request for UserReq {
    type Output = User;
    fn queue(&self) -> Self::Output {
        User::default()
    }
}

impl UserReq {
    pub fn new() -> Self {
        Self {
            user_id: None,
            username: None,
            mode: None,
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
