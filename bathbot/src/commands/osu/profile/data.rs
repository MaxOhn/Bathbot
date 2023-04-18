use std::{
    cmp::{Ordering, Reverse},
    collections::HashMap,
    hint,
};

use bathbot_model::rosu_v2::user::User;
use bathbot_util::{osu::BonusPP, IntHasher};
use eyre::Report;
use eyre::Result;
use rosu_v2::prelude::{GameMod, GameModIntermode, GameModsIntermode, Score, Username};
use time::UtcOffset;
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::osu::MinMaxAvg,
    core::Context,
    manager::redis::{osu::UserArgsSlim, RedisData},
};

pub struct ProfileData {
    pub user: RedisData<User>,
    pub discord_id: Option<Id<UserMarker>>,
    pub tz: Option<UtcOffset>,
    pub skin_url: Availability<Option<String>>,
    scores: Availability<Vec<Score>>,
    score_rank: Availability<u32>,
    top100stats: Option<Top100Stats>,
    mapper_names: Availability<HashMap<u32, Username, IntHasher>>,
}

impl ProfileData {
    pub(crate) fn new(
        user: RedisData<User>,
        discord_id: Option<Id<UserMarker>>,
        tz: Option<UtcOffset>,
    ) -> Self {
        Self {
            user,
            discord_id,
            tz,
            skin_url: Availability::NotRequested,
            scores: Availability::NotRequested,
            score_rank: Availability::NotRequested,
            top100stats: None,
            mapper_names: Availability::NotRequested,
        }
    }

    pub(crate) async fn skin_url(&mut self, ctx: &Context) -> Option<String> {
        match self.skin_url {
            Availability::Received(ref skin_url) => return skin_url.to_owned(),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let skin_fut = ctx.user_config().skin_from_osu_id(self.user.user_id());

        match skin_fut.await {
            Ok(skin_url) => self.skin_url.insert(skin_url).to_owned(),
            Err(err) => {
                warn!("{err:?}");
                self.skin_url = Availability::Errored;

                None
            }
        }
    }

    pub(crate) async fn score_rank(&mut self, ctx: &Context) -> Option<u32> {
        match self.score_rank {
            Availability::Received(rank) => return Some(rank),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let (user_id, mode) = match &self.user {
            RedisData::Original(user) => (user.user_id, user.mode),
            RedisData::Archive(user) => (user.user_id, user.mode),
        };

        let user_fut = ctx.client().get_respektive_user(user_id, mode);

        match user_fut.await {
            Ok(Some(user)) => Some(*self.score_rank.insert(user.rank)),
            Ok(None) => {
                self.score_rank = Availability::Errored;

                None
            }
            Err(err) => {
                warn!("{:?}", err.wrap_err("failed to get respektive user"));
                self.score_rank = Availability::Errored;

                None
            }
        }
    }

    pub(crate) async fn bonus_pp(&mut self, ctx: &Context) -> Option<f32> {
        let scores = self.get_scores(ctx).await?;

        let mut bonus_pp = BonusPP::new();

        for (i, score) in scores.iter().enumerate() {
            if let Some(weight) = score.weight {
                bonus_pp.update(weight.pp, i);
            }
        }

        Some(bonus_pp.calculate(self.user.stats()))
    }

    pub(crate) async fn top100stats(&mut self, ctx: &Context) -> Option<&Top100Stats> {
        if let Some(ref stats) = self.top100stats {
            return Some(stats);
        }

        let scores = self.get_scores(ctx).await?;

        match Top100Stats::new(ctx, scores).await {
            Ok(stats) => Some(self.top100stats.insert(stats)),
            Err(err) => {
                warn!("{:?}", err.wrap_err("failed to calculate top100 stats"));

                None
            }
        }
    }

    pub(crate) async fn top100mods(&mut self, ctx: &Context) -> Option<Top100Mods> {
        self.get_scores(ctx).await.map(Top100Mods::new)
    }

    pub(crate) async fn top100mappers<'s>(
        &'s mut self,
        ctx: &Context,
    ) -> Option<Vec<MapperEntry<'s>>> {
        let mut entries: Vec<_> = {
            let scores = self.get_scores(ctx).await?;
            let mut entries = HashMap::with_capacity_and_hasher(32, IntHasher);

            for score in scores {
                if let Some(ref map) = score.map {
                    let (count, pp) = entries.entry(map.creator_id).or_insert((0, 0.0));

                    *count += 1;

                    if let Some(weight) = score.weight {
                        *pp += weight.pp;
                    }
                }
            }

            entries.into_iter().collect()
        };

        entries.sort_unstable_by(|(_, (count_a, pp_a)), (_, (count_b, pp_b))| {
            count_b
                .cmp(count_a)
                .then_with(|| pp_b.partial_cmp(pp_a).unwrap_or(Ordering::Equal))
        });

        entries.truncate(10);

        let mapper_names = match self.mapper_names {
            Availability::Received(ref names) => names,
            Availability::Errored => return None,
            Availability::NotRequested => {
                let ids: Vec<_> = entries.iter().map(|(id, _)| *id as i32).collect();

                let mut names = match ctx.osu_user().names(&ids).await {
                    Ok(names) => names,
                    Err(err) => {
                        warn!("{:?}", err.wrap_err("failed to get mapper names"));

                        HashMap::default()
                    }
                };

                if names.len() != ids.len() {
                    for (id, _) in entries.iter() {
                        if names.contains_key(id) {
                            continue;
                        }

                        let mode = self.user.mode();

                        let user = match ctx.osu().user(*id).mode(mode).await {
                            Ok(user) => user,
                            Err(err) => {
                                let err = Report::new(err).wrap_err("failed to get user");
                                warn!("{err:?}");

                                continue;
                            }
                        };

                        if let Err(err) = ctx.osu_user().store_user(&user, mode).await {
                            warn!("{:?}", err.wrap_err("failed to upsert user"));
                        }

                        names.insert(user.user_id, user.username);
                    }
                }

                self.mapper_names.insert(names)
            }
        };

        let mappers = entries
            .into_iter()
            .map(|(id, (count, pp))| MapperEntry {
                name: mapper_names
                    .get(&id)
                    .map_or("<unknown name>", Username::as_str),
                pp,
                count,
            })
            .collect();

        Some(mappers)
    }

    pub async fn own_maps_in_top100(&mut self, ctx: &Context) -> Option<usize> {
        let user_id = self.user.user_id();
        let scores = self.get_scores(ctx).await?;

        let count = scores.iter().fold(0, |count, score| {
            let self_mapped = score
                .map
                .as_ref()
                .map_or(0, |map| (map.creator_id == user_id) as usize);

            count + self_mapped
        });

        Some(count)
    }

    async fn get_scores(&mut self, ctx: &Context) -> Option<&[Score]> {
        match self.scores {
            Availability::Received(ref scores) => return Some(scores),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let (user_id, mode) = match &self.user {
            RedisData::Original(user) => (user.user_id, user.mode),
            RedisData::Archive(user) => (user.user_id, user.mode),
        };

        let user_args = UserArgsSlim::user_id(user_id).mode(mode);

        match ctx.osu_scores().top().exec(user_args).await {
            Ok(scores) => Some(self.scores.insert(scores)),
            Err(err) => {
                let wrap = "failed to get top scores";
                warn!("{:?}", Report::new(err).wrap_err(wrap));
                self.scores = Availability::Errored;

                None
            }
        }
    }
}

#[derive(Copy, Clone)]
pub enum Availability<T> {
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

pub struct Top100Stats {
    pub acc: MinMaxAvg<f32>,
    pub combo: MinMaxAvg<u32>,
    pub misses: MinMaxAvg<u32>,
    pub pp: MinMaxAvg<f32>,
    pub stars: MinMaxAvg<f64>,
    pub ar: MinMaxAvg<f64>,
    pub cs: MinMaxAvg<f64>,
    pub hp: MinMaxAvg<f64>,
    pub od: MinMaxAvg<f64>,
    pub bpm: MinMaxAvg<f32>,
    pub len: MinMaxAvg<f32>,
}

impl Top100Stats {
    pub async fn new(ctx: &Context, scores: &[Score]) -> Result<Self> {
        let maps_id_checksum = scores
            .iter()
            .map(|score| {
                let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());

                (score.map_id as i32, checksum)
            })
            .collect();

        let maps = ctx.osu_map().maps(&maps_id_checksum).await?;

        let mut this = Self {
            acc: MinMaxAvg::new(),
            combo: MinMaxAvg::new(),
            misses: MinMaxAvg::new(),
            pp: MinMaxAvg::new(),
            stars: MinMaxAvg::new(),
            ar: MinMaxAvg::new(),
            cs: MinMaxAvg::new(),
            hp: MinMaxAvg::new(),
            od: MinMaxAvg::new(),
            bpm: MinMaxAvg::new(),
            len: MinMaxAvg::new(),
        };

        for score in scores {
            this.acc.add(score.accuracy);
            this.combo.add(score.max_combo);
            this.misses.add(score.statistics.count_miss);

            let map = score
                .map
                .as_ref()
                .and_then(|map| maps.get(&map.map_id))
                .expect("missing map");

            let mut calc = ctx.pp(map).mode(score.mode).mods(score.mods.bits());

            let stars = calc.difficulty().await.stars();
            this.stars.add(stars);

            let pp = match score.pp {
                Some(pp) => pp,
                None => calc.score(score).performance().await.pp() as f32,
            };

            this.pp.add(pp);

            let map_attrs = map
                .pp_map
                .attributes()
                .mods(score.mods.bits())
                .converted(map.mode() != score.mode)
                .build();

            this.ar.add(map_attrs.ar);
            this.cs.add(map_attrs.cs);
            this.hp.add(map_attrs.hp);
            this.od.add(map_attrs.od);
            this.bpm.add(map.bpm() * map_attrs.clock_rate as f32);
            this.len
                .add(map.seconds_drain() as f32 / map_attrs.clock_rate as f32);
        }

        Ok(this)
    }
}

pub struct Top100Mods {
    pub percent_mods: Vec<(GameModIntermode, u8)>,
    pub percent_mod_comps: Vec<(GameModsIntermode, u8)>,
    pub pp_mod_comps: Vec<(GameModsIntermode, f32)>,
}

impl Top100Mods {
    fn new(scores: &[Score]) -> Self {
        let mut percent_mods = HashMap::with_hasher(IntHasher);
        let mut percent_mod_comps = HashMap::new();
        let mut pp_mod_comps = HashMap::<_, f32, _>::new();

        for score in scores {
            let mods: GameModsIntermode = score.mods.iter().map(GameMod::intermode).collect();

            if let Some(weight) = score.weight {
                *pp_mod_comps.entry(mods.clone()).or_default() += weight.pp;
            }

            *percent_mod_comps.entry(mods).or_default() += 1;

            for m in score.mods.iter().map(GameMod::intermode) {
                *percent_mods.entry(m).or_default() += 1;
            }
        }

        let mut percent_mods: Vec<_> = percent_mods.into_iter().collect();
        percent_mods.sort_unstable_by_key(|(_, percent)| Reverse(*percent));

        let mut percent_mod_comps: Vec<_> = percent_mod_comps.into_iter().collect();
        percent_mod_comps.sort_unstable_by_key(|(_, percent)| Reverse(*percent));

        let mut pp_mod_comps: Vec<_> = pp_mod_comps.into_iter().collect();
        pp_mod_comps.sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        Self {
            percent_mods,
            percent_mod_comps,
            pp_mod_comps,
        }
    }
}

pub struct MapperEntry<'n> {
    pub name: &'n str,
    pub pp: f32,
    pub count: u8,
}
