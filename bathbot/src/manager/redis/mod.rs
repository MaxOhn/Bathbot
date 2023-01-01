use std::fmt::Write;

use bb8_redis::redis::AsyncCommands;
use eyre::{Report, Result};
use rkyv::{ser::serializers::AllocSerializer, Serialize};
use rosu_v2::prelude::{GameMode, OsuError, Rankings};

use crate::{
    core::{Context, Redis},
    custom_client::{
        OsekaiBadge, OsekaiMedal, OsekaiRanking, OsuTrackerIdCount, OsuTrackerPpGroup,
        OsuTrackerStats,
    },
};

pub use self::data::{ArchivedBytes, RedisData};

pub mod cache;
pub mod osu;

mod data;

type RedisResult<T, E = Report> = Result<RedisData<T>, E>;

#[derive(Copy, Clone)]
pub struct RedisManager<'c> {
    ctx: &'c Context,
    redis: &'c Redis,
}

impl<'c> RedisManager<'c> {
    pub fn new(ctx: &'c Context, redis: &'c Redis) -> Self {
        Self { ctx, redis }
    }

    pub async fn badges(self) -> RedisResult<Vec<OsekaiBadge>> {
        const EXPIRE_SECONDS: usize = 7200;
        const KEY: &str = "osekai_badges";

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(KEY).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_badges();
                    trace!("Found badges in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let badges = self.ctx.client().get_osekai_badges().await?;

        if let Some(mut conn) = conn {
            let bytes = rkyv::to_bytes::<_, 200_000>(&badges).expect("failed to serialize badges");
            let set_fut = conn.set_ex::<_, _, ()>(KEY, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(badges))
    }

    pub async fn medals(self) -> RedisResult<Vec<OsekaiMedal>> {
        const EXPIRE_SECONDS: usize = 3600;
        const KEY: &str = "osekai_medals";

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(KEY).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_medals();
                    trace!("Found medals in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let medals = self.ctx.client().get_osekai_medals().await?;

        if let Some(mut conn) = conn {
            let bytes = rkyv::to_bytes::<_, 80_000>(&medals).expect("failed to serialize medals");
            let set_fut = conn.set_ex::<_, _, ()>(KEY, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(medals))
    }

    pub async fn osekai_ranking<R>(self) -> RedisResult<Vec<R::Entry>>
    where
        R: OsekaiRanking,
        <R as OsekaiRanking>::Entry: Serialize<AllocSerializer<70_000>>,
    {
        const EXPIRE_SECONDS: usize = 7200;
        let key = format!("osekai_ranking_{}", R::FORM);

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(&key).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_osekai_ranking();
                    trace!("Found osekai ranking in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let ranking = self.ctx.client().get_osekai_ranking::<R>().await?;

        if let Some(mut conn) = conn {
            let bytes =
                rkyv::to_bytes::<_, 70_000>(&ranking).expect("failed to serialize osekai ranking");

            let set_fut = conn.set_ex::<_, _, ()>(key, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(ranking))
    }

    pub async fn osutracker_pp_group(self, pp: u32) -> RedisResult<OsuTrackerPpGroup> {
        const EXPIRE_SECONDS: usize = 86_400;
        let key = format!("osutracker_pp_group_{pp}");

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(&key).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_osutracker_pp_group();
                    trace!("Found osutracker pp group in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let group = self.ctx.client().get_osutracker_pp_group(pp).await?;

        if let Some(mut conn) = conn {
            let bytes = rkyv::to_bytes::<_, 7_000>(&group)
                .expect("failed to serialize osutracker pp groups");

            let set_fut = conn.set_ex::<_, _, ()>(key, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(group))
    }

    pub async fn osutracker_stats(self) -> RedisResult<OsuTrackerStats> {
        const EXPIRE_SECONDS: usize = 86_400;
        const KEY: &str = "osutracker_stats";

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(KEY).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_osutracker_stats();
                    trace!("Found osutracker stats in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let stats = self.ctx.client().get_osutracker_stats().await?;

        if let Some(mut conn) = conn {
            let bytes =
                rkyv::to_bytes::<_, 190_000>(&stats).expect("failed to serialize osutracker stats");

            let set_fut = conn.set_ex::<_, _, ()>(KEY, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(stats))
    }

    pub async fn osutracker_counts(self) -> RedisResult<Vec<OsuTrackerIdCount>> {
        const EXPIRE_SECONDS: usize = 86_400;
        const KEY: &str = "osutracker_id_counts";

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(KEY).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_osutracker_counts();
                    trace!("Found osutracker counts in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let counts = self.ctx.client().get_osutracker_counts().await?;

        if let Some(mut conn) = conn {
            let bytes = rkyv::to_bytes::<_, 330_000>(&counts)
                .expect("failed to serialize osutracker counts");

            let set_fut = conn.set_ex::<_, _, ()>(KEY, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(counts))
    }

    pub async fn pp_ranking(
        self,
        mode: GameMode,
        page: u32,
        country: Option<&str>,
    ) -> RedisResult<Rankings, OsuError> {
        const EXPIRE_SECONDS: usize = 1800;
        let mut key = format!("pp_ranking_{}_{page}", mode as u8);

        if let Some(country) = country {
            let _ = write!(key, "_{country}");
        }

        let conn = match self.redis.get().await {
            Ok(mut conn) => match conn.get::<_, Vec<u8>>(&key).await {
                Ok(bytes) if bytes.is_empty() => Some(conn),
                Ok(bytes) => {
                    self.ctx.stats.inc_cached_pp_ranking();
                    trace!("Found pp ranking in cache ({} bytes)", bytes.len());

                    return Ok(RedisData::new_archived(bytes));
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get bytes");
                    warn!("{report:?}");

                    Some(conn)
                }
            },
            Err(err) => {
                let report = Report::new(err).wrap_err("Failed to get redis connection");
                warn!("{report:?}");

                None
            }
        };

        let ranking_fut = self.ctx.osu().performance_rankings(mode).page(page);

        let ranking = if let Some(country) = country {
            ranking_fut.country(country).await?
        } else {
            ranking_fut.await?
        };

        if let Some(mut conn) = conn {
            let bytes = rkyv::to_bytes::<_, 40_000>(&ranking).expect("failed to serialize ranking");
            let set_fut = conn.set_ex::<_, _, ()>(key, bytes.as_slice(), EXPIRE_SECONDS);

            if let Err(err) = set_fut.await {
                let report = Report::new(err).wrap_err("Failed to insert bytes into cache");
                warn!("{report:?}");
            }
        }

        Ok(RedisData::new(ranking))
    }
}
