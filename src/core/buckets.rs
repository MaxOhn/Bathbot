use chrono::Utc;
use dashmap::DashMap;
use std::{collections::HashMap, hash::Hash, str::FromStr};
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
    #[inline]
    fn new(ratelimit: Ratelimit) -> Self {
        Self {
            ratelimit,
            users: HashMap::new(),
        }
    }

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
    All,
    Songs,
    BgStart,
    BgBigger,
    BgHint,
    Snipe,
}

impl FromStr for BucketName {
    type Err = &'static str;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let bucket = match name {
            "all" => BucketName::All,
            "songs" => BucketName::Songs,
            "bg_start" => BucketName::BgStart,
            "bg_bigger" => BucketName::BgBigger,
            "bg_hint" => BucketName::BgHint,
            "snipe" => BucketName::Snipe,
            _ => return Err("Unknown bucket name"),
        };

        Ok(bucket)
    }
}

pub fn buckets() -> Buckets {
    let buckets = DashMap::new();

    let insert_bucket = |name, delay, time_span, limit| {
        let ratelimit = Ratelimit {
            delay,
            limit: Some((time_span, limit)),
        };

        buckets.insert(name, Mutex::new(Bucket::new(ratelimit)));
    };

    insert_bucket(BucketName::All, 0, 9, 4);
    insert_bucket(BucketName::Songs, 20, 0, 1);
    insert_bucket(BucketName::BgStart, 2, 20, 3);
    insert_bucket(BucketName::BgBigger, 1, 8, 2);
    insert_bucket(BucketName::BgHint, 0, 10, 4);
    insert_bucket(BucketName::Snipe, 0, 600, 10);

    buckets
}
