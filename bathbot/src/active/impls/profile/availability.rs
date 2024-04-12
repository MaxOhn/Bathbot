use std::{collections::HashMap, hint, iter, num::NonZeroU32, sync::Arc};

use bathbot_model::{RankAccPeaks, RespektiveUserRankHighest};
use bathbot_util::IntHasher;
use rosu_v2::prelude::{GameMode, Score, Username};

use crate::{
    core::{Context, ContextExt},
    manager::redis::osu::UserArgsSlim,
};

#[derive(Copy, Clone)]
pub(super) enum Availability<T> {
    Received(T),
    Errored,
    NotRequested,
}

impl<T> Availability<T> {
    fn insert(&mut self, value: T) -> &mut T {
        *self = Self::Received(value);

        match self {
            Availability::Received(val) => val,
            // SAFETY: the code above just filled in a value
            _ => unsafe { hint::unreachable_unchecked() },
        }
    }
}

impl Availability<Box<[Score]>> {
    pub(super) async fn get(
        &mut self,
        ctx: Arc<Context>,
        user_id: u32,
        mode: GameMode,
        legacy_scores: bool,
    ) -> Option<&[Score]> {
        match self {
            Self::Received(ref scores) => return Some(scores),
            Self::Errored => return None,
            Self::NotRequested => {}
        }

        let user_args = UserArgsSlim::user_id(user_id).mode(mode);

        match ctx.osu_scores().top(legacy_scores).exec(user_args).await {
            Ok(scores) => Some(self.insert(scores.into_boxed_slice())),
            Err(err) => {
                warn!(?err, "Failed to get top scores");
                *self = Availability::Errored;

                None
            }
        }
    }
}

pub(super) struct MapperNames(pub HashMap<u32, Username, IntHasher>);

impl Availability<MapperNames> {
    pub(super) async fn get(
        &mut self,
        ctx: Arc<Context>,
        mode: GameMode,
        entries: &[(u32, (u8, f32))],
    ) -> Option<&MapperNames> {
        match self {
            Availability::Received(ref names) => Some(names),
            Availability::Errored => None,
            Availability::NotRequested => {
                let ids: Vec<_> = entries.iter().map(|(id, _)| *id as i32).collect();

                let mut names = match ctx.osu_user().names(&ids).await {
                    Ok(names) => names,
                    Err(err) => {
                        warn!(?err, "Failed to get mapper names");

                        HashMap::default()
                    }
                };

                if names.len() != ids.len() {
                    for (id, _) in entries.iter() {
                        if names.contains_key(id) {
                            continue;
                        }

                        // TODO: use ctx.osu().users(iter)
                        let user = match ctx.osu().user(*id).mode(mode).await {
                            Ok(user) => user,
                            Err(err) => {
                                warn!(?err, "Failed to get user");

                                continue;
                            }
                        };

                        let user_id = user.user_id;
                        let username = user.username.clone();

                        let ctx = ctx.cloned();
                        tokio::spawn(async move {
                            ctx.osu_user().store(&user, mode).await;
                        });

                        names.insert(user_id, username);
                    }
                }

                Some(self.insert(MapperNames(names)))
            }
        }
    }
}

pub(super) struct SkinUrl(pub Option<String>);

impl Availability<SkinUrl> {
    pub(super) async fn get(&mut self, ctx: &Context, user_id: u32) -> Option<&str> {
        match self {
            Availability::Received(SkinUrl(ref skin_url)) => return skin_url.as_deref(),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let skin_fut = ctx.user_config().skin_from_osu_id(user_id);

        match skin_fut.await {
            Ok(skin_url) => {
                let SkinUrl(skin_url) = self.insert(SkinUrl(skin_url));

                skin_url.as_deref()
            }
            Err(err) => {
                warn!("{err:?}");
                *self = Availability::Errored;

                None
            }
        }
    }
}

#[derive(Copy, Clone)]
pub(super) struct ScoreData {
    pub rank: Option<NonZeroU32>,
    pub highest_rank: Option<RespektiveUserRankHighest>,
}

impl Availability<ScoreData> {
    pub(super) async fn get(
        &mut self,
        ctx: &Context,
        user_id: u32,
        mode: GameMode,
    ) -> Option<ScoreData> {
        match self {
            Availability::Received(data) => return Some(*data),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let user_fut = ctx.client().get_respektive_users(iter::once(user_id), mode);

        match user_fut.await.map(|mut iter| iter.next().flatten()) {
            Ok(Some(user)) => {
                let data = ScoreData {
                    rank: user.rank,
                    highest_rank: user.rank_highest,
                };

                self.insert(data);

                Some(data)
            }
            Ok(None) => {
                *self = Availability::Errored;

                None
            }
            Err(err) => {
                warn!(?err, "Failed to get respektive user");
                *self = Availability::Errored;

                None
            }
        }
    }
}

impl Availability<RankAccPeaks> {
    pub(super) async fn get(
        &mut self,
        ctx: &Context,
        user_id: u32,
        mode: GameMode,
    ) -> Option<RankAccPeaks> {
        match self {
            Availability::Received(ref peaks) => return Some(peaks.to_owned()),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let peaks_fut = ctx.client().osu_user_rank_acc_peak(user_id, mode);

        match peaks_fut.await {
            Ok(Some(peaks)) => {
                self.insert(peaks.clone());

                Some(peaks)
            }
            Ok(None) => {
                *self = Availability::Errored;

                None
            }
            Err(err) => {
                warn!(?err, "Failed to get osutrack peaks");
                *self = Availability::Errored;

                None
            }
        }
    }
}
