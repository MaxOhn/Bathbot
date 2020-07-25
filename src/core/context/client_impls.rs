use crate::{BotResult, Context};

use rosu::{
    backend::UserRequest,
    models::{GameMode, User},
    OsuResult,
};

impl Context {
    pub async fn osu_user(&self, name: &str, mode: GameMode) -> OsuResult<Option<User>> {
        let osu = &self.clients.osu;
        let req = UserRequest::with_username(name).mode(mode);
        req.queue_single(osu).await
    }
}
