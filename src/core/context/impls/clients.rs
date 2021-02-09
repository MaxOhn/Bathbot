use crate::{Context, Database};

use rosu::Osu;

impl Context {
    #[inline]
    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    #[inline]
    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }
}
