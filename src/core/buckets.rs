use chrono::Utc;
use dashmap::DashMap;
use std::{collections::HashMap, hash::Hash};
use tokio::sync::Mutex;

pub type Buckets = DashMap<BucketName, Mutex<Bucket>>;

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

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub enum BucketName {
    // All,
    Songs,
    BgStart,
    BgBigger,
    BgHint,
    Snipe,
}

impl From<&str> for BucketName {
    fn from(s: &str) -> Self {
        match s {
            // "all" => BucketName::All,
            "songs" => BucketName::Songs,
            "bg_start" => BucketName::BgStart,
            "bg_bigger" => BucketName::BgBigger,
            "bg_hint" => BucketName::BgHint,
            "snipe" => BucketName::Snipe,
            _ => panic!("No bucket called `{}`", s),
        }
    }
}

pub fn buckets() -> Buckets {
    let buckets = DashMap::new();
    // insert_bucket(&buckets, BucketName::All, 0, 60, 30);
    insert_bucket(&buckets, BucketName::Songs, 20, 0, 1);
    insert_bucket(&buckets, BucketName::BgStart, 2, 20, 3);
    insert_bucket(&buckets, BucketName::BgBigger, 1, 10, 3);
    insert_bucket(&buckets, BucketName::BgHint, 0, 10, 4);
    insert_bucket(&buckets, BucketName::Snipe, 0, 600, 5);
    buckets
}

fn insert_bucket(buckets: &Buckets, name: BucketName, delay: i64, time_span: i64, limit: i32) {
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
