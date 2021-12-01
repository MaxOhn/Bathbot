use chrono::Utc;
use hashbrown::HashMap;
use parking_lot::Mutex;
use std::{hash::Hash, str::FromStr};

pub struct Buckets([Mutex<Bucket>; 7]);

impl Buckets {
    pub fn new() -> Self {
        let make_bucket = |delay, time_span, limit| {
            let ratelimit = Ratelimit {
                delay,
                limit: Some((time_span, limit)),
            };

            Mutex::new(Bucket::new(ratelimit))
        };

        Self([
            make_bucket(0, 9, 4),   // All
            make_bucket(1, 8, 2),   // BgBigger
            make_bucket(0, 10, 4),  // BgHint
            make_bucket(2, 20, 3),  // BgStart
            make_bucket(5, 900, 3), // MatchLive
            make_bucket(0, 60, 10), // Snipe
            make_bucket(20, 0, 1),  // Songs
        ])
    }

    pub fn get(&self, bucket: BucketName) -> &Mutex<Bucket> {
        match bucket {
            BucketName::All => &self.0[0],
            BucketName::BgBigger => &self.0[1],
            BucketName::BgHint => &self.0[2],
            BucketName::BgStart => &self.0[3],
            BucketName::MatchLive => &self.0[4],
            BucketName::Snipe => &self.0[5],
            BucketName::Songs => &self.0[6],
        }
    }
}

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
    BgBigger,
    BgHint,
    BgStart,
    MatchLive,
    Snipe,
    Songs,
}

impl FromStr for BucketName {
    type Err = &'static str;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let bucket = match name {
            "all" => BucketName::All,
            "bg_bigger" => BucketName::BgBigger,
            "bg_hint" => BucketName::BgHint,
            "bg_start" => BucketName::BgStart,
            "match_live" => BucketName::MatchLive,
            "snipe" => BucketName::Snipe,
            "songs" => BucketName::Songs,
            _ => return Err("Unknown bucket name"),
        };

        Ok(bucket)
    }
}
