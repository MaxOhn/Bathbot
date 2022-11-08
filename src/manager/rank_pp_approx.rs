use bathbot_psql::Database;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;

#[derive(Copy, Clone)]
pub struct ApproxManager<'d> {
    psql: &'d Database,
}

impl<'d> ApproxManager<'d> {
    pub fn new(psql: &'d Database) -> Self {
        Self { psql }
    }

    pub async fn rank(self, pp: f32, mode: GameMode) -> Result<u32> {
        self.psql
            .select_rank_approx_by_pp(pp, mode)
            .await
            .wrap_err("failed to approximate rank")
    }

    pub async fn pp(self, rank: u32, mode: GameMode) -> Result<f32> {
        self.psql
            .select_pp_approx_by_rank(rank, mode)
            .await
            .wrap_err("failed to approximate pp")
    }
}
