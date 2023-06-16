use std::{collections::HashMap, hash::Hash};

use bathbot_util::IntHasher;
use parking_lot::Mutex;
use time::OffsetDateTime;

pub struct Buckets([Mutex<Bucket>; 8]);

impl Buckets {
    #[allow(clippy::new_without_default)]
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
            make_bucket(2, 20, 3),  // BgSkip
            make_bucket(15, 0, 1),  // MatchCompare
            make_bucket(5, 900, 3), // MatchLive
            make_bucket(0, 600, 2), // Render
            make_bucket(20, 0, 1),  // Songs
        ])
    }

    pub fn get(&self, bucket: BucketName) -> &Mutex<Bucket> {
        match bucket {
            BucketName::All => &self.0[0],
            BucketName::BgBigger => &self.0[1],
            BucketName::BgHint => &self.0[2],
            BucketName::BgSkip => &self.0[3],
            BucketName::MatchCompare => &self.0[4],
            BucketName::MatchLive => &self.0[5],
            BucketName::Render => &self.0[6],
            BucketName::Songs => &self.0[7],
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
    pub users: HashMap<u64, MemberRatelimit, IntHasher>,
}

impl Bucket {
    fn new(ratelimit: Ratelimit) -> Self {
        Self {
            ratelimit,
            users: HashMap::default(),
        }
    }

    pub fn take(&mut self, user_id: u64) -> i64 {
        let time = OffsetDateTime::now_utc().unix_timestamp();

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
    BgSkip,
    MatchCompare,
    MatchLive,
    Render,
    Songs,
}
