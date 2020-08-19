use crate::{Context, Database};

use rosu::{
    backend::UserRequest,
    models::{GameMode, User},
    Osu, OsuResult,
};

impl Context {
    pub async fn osu_user(&self, name: &str, mode: GameMode) -> OsuResult<Option<User>> {
        UserRequest::with_username(name)?
            .mode(mode)
            .queue_single(&self.clients.osu)
            .await
    }

    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }
}
