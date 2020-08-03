use chrono::Utc;
use dashmap::DashMap;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub type Buckets = DashMap<&'static str, Mutex<Bucket>>;

pub struct Ratelimit {
    pub delay: i64,
    pub limit: Option<(i64, i32)>,
}

#[derive(Default)]
pub struct MemberRatelimit {
    pub last_time: i64,
    pub set_time: i64,
    pub tickets: i32,
}

pub struct Bucket {
    pub ratelimit: Ratelimit,
    pub users: HashMap<u64, MemberRatelimit>,
}

impl Bucket {
    pub fn take(&mut self, user_id: u64) -> i64 {
        let time = Utc::now().timestamp();
        let user = self
            .users
            .entry(user_id)
            .or_insert_with(MemberRatelimit::default);
        if let Some((timespan, limit)) = self.ratelimit.limit {
            if (user.tickets + 1) > limit {
                if time < (user.set_time + timespan) {
                    return (user.set_time + timespan) - time;
                } else {
                    user.tickets = 0;
                    user.set_time = time;
                }
            }
        }
        if time < user.last_time + self.ratelimit.delay {
            (user.last_time + self.ratelimit.delay) - time
        } else {
            user.tickets += 1;
            user.last_time = time;
            0
        }
    }
}

pub fn buckets() -> Buckets {
    let buckets = DashMap::new();
    insert_bucket(&buckets, "songs", 20, 0, 1);
    insert_bucket(&buckets, "bg_start", 2, 20, 3);
    insert_bucket(&buckets, "bg_bigger", 1, 10, 3);
    insert_bucket(&buckets, "bg_hint", 1, 5, 2);
    buckets
}

fn insert_bucket(buckets: &Buckets, name: &'static str, delay: i64, time_span: i64, limit: i32) {
    buckets.insert(
        name,
        Mutex::new(Bucket {
            ratelimit: Ratelimit {
                delay,
                limit: Some((time_span, limit)),
            },
            users: HashMap::new(),
        }),
    );
}
