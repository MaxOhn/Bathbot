use crate::{core::stored_values::Values, Context};

use rosu::model::GameMode;
use tokio::sync::Mutex;

impl Context {
    pub fn stars(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.data.stored_values.mania_stars,
            GameMode::CTB => &self.data.stored_values.ctb_stars,
            _ => unreachable!(),
        }
    }

    pub fn pp_lock(&self) -> &Mutex<()> {
        &self.data.perf_calc_mutex
    }
}
