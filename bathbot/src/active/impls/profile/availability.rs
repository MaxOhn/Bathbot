use std::{collections::HashMap, hint, iter};

use bathbot_util::IntHasher;
use rosu_v2::prelude::{GameMode, Score, Username};

use crate::{core::Context, manager::redis::osu::UserArgsSlim};

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
        ctx: &Context,
        user_id: u32,
        mode: GameMode,
    ) -> Option<&[Score]> {
        match self {
            Self::Received(ref scores) => return Some(scores),
            Self::Errored => return None,
            Self::NotRequested => {}
        }

        let user_args = UserArgsSlim::user_id(user_id).mode(mode);

        match ctx.osu_scores().top().exec(user_args).await {
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
        ctx: &Context,
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

                        let user = match ctx.osu().user(*id).mode(mode).await {
                            Ok(user) => user,
                            Err(err) => {
                                warn!(?err, "Failed to get user");

                                continue;
                            }
                        };

                        if let Err(err) = ctx.osu_user().store_user(&user, mode).await {
                            warn!(?err, "Failed to upsert user");
                        }

                        names.insert(user.user_id, user.username);
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

pub(super) struct ScoreRank(pub u32);

impl Availability<ScoreRank> {
    pub(super) async fn get(&mut self, ctx: &Context, user_id: u32, mode: GameMode) -> Option<u32> {
        match self {
            Availability::Received(ScoreRank(rank)) => return Some(*rank),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let user_fut = ctx.client().get_respektive_users(iter::once(user_id), mode);

        match user_fut.await {
            Ok(mut iter) => match iter.next().flatten().and_then(|user| user.rank) {
                Some(rank) => {
                    let ScoreRank(rank) = self.insert(ScoreRank(rank.get()));

                    Some(*rank)
                }
                None => {
                    *self = Availability::Errored;

                    None
                }
            },
            Err(err) => {
                warn!(?err, "Failed to get respektive user");
                *self = Availability::Errored;

                None
            }
        }
    }
}
