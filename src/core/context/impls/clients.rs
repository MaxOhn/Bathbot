use crate::{Context, Database};

use rosu::Osu;

impl Context {
    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }
}
