use std::{collections::HashMap, mem, sync::Mutex, time::Duration};

use bathbot_util::IntHasher;
use rosu_v2::prelude::Grade;

use crate::core::Context;

pub struct Gamba;

static PENDING_USERS: Mutex<HashMap<u32, u64, IntHasher>> =
    Mutex::new(HashMap::with_hasher(IntHasher));

impl Gamba {
    pub fn spawn_listener() {
        tokio::spawn(run_listener());
    }

    pub(super) fn process_score(user_id: u32, grade: Grade) {
        if grade == Grade::F {
            return;
        }

        *PENDING_USERS.lock().unwrap().entry(user_id).or_insert(0) += 1;
    }
}

async fn run_listener() {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut buf = HashMap::with_hasher(IntHasher);
    let psql = Context::psql();

    loop {
        interval.tick().await;

        mem::swap(&mut *PENDING_USERS.lock().unwrap(), &mut buf);

        if buf.is_empty() {
            continue;
        }

        if let Err(err) = psql.increase_multi_bathcoins(&buf).await {
            warn!(?err, "Failed to increase multi bathcoins");
        }

        buf.clear();
    }
}
