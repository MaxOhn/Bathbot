use std::time::Duration;

use leaky_bucket_lite::LeakyBucket;

pub(super) struct Ratelimiters {
    inner: Box<[LeakyBucket]>,
}

impl Ratelimiters {
    pub fn new() -> Self {
        Self {
            inner: make_buckets(),
        }
    }

    pub fn get(&self, site: Site) -> &LeakyBucket {
        &self.inner[site as usize]
    }
}

/// List of `{variant name} -> {allowed requests per second}`
macro_rules! sites {
    ( $( $variant:ident -> $per_second:literal, )+ ) => {
        #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
        #[repr(u8)]
        pub enum Site {
            $( $variant, )*
        }

        impl Site {
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => stringify!($variant), )*
                }
            }
        }

        fn make_buckets() -> Box<[LeakyBucket]> {
            let make_bucket = |per_second| {
                LeakyBucket::builder()
                    .max(per_second)
                    .tokens(per_second)
                    .refill_interval(Duration::from_millis(1000 / per_second as u64))
                    .refill_amount(1)
                    .build()
            };

            vec![$( make_bucket($per_second), )*].into_boxed_slice()
        }
    };
}

sites! {
    DiscordAttachment -> 2,
    Flags -> 10,
    Github -> 5,
    Huismetbenen -> 2,
    KittenRoleplay -> 5,
    MissAnalyzer -> 5,
    Osekai -> 2,
    OsuAvatar -> 10,
    OsuBadge -> 10,
    OsuMapFile -> 2,
    OsuMapsetCover -> 10,
    OsuMedalIcon -> 25,
    OsuProfile -> 1,
    OsuStats -> 2,
    OsuTrack -> 2,
    Relax -> 2,
    Respektive -> 1,
    Twitch -> 5,
}
