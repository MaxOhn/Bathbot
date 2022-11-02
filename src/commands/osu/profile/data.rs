use std::{
    cmp::{Ordering, Reverse},
    hint,
};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMods, Score, User, Username};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::osu::{get_scores, MinMaxAvg, ScoreArgs, UserArgs},
    core::Context,
    pp::PpCalculator,
    util::{hasher::IntHasher, osu::BonusPP},
};

pub struct ProfileData {
    pub user: User,
    pub author_id: Option<Id<UserMarker>>,
    scores: Availability<Vec<Score>>,
    score_rank: Availability<u32>,
    top100stats: Option<Top100Stats>,
    mapper_names: Availability<HashMap<u32, Username, IntHasher>>,
}

impl ProfileData {
    pub(crate) fn new(user: User, author_id: Option<Id<UserMarker>>) -> Self {
        Self {
            user,
            author_id,
            scores: Availability::NotRequested,
            score_rank: Availability::NotRequested,
            top100stats: None,
            mapper_names: Availability::NotRequested,
        }
    }

    pub(crate) async fn score_rank(&mut self, ctx: &Context) -> Option<u32> {
        match self.score_rank {
            Availability::Received(rank) => return Some(rank),
            Availability::Errored => return None,
            Availability::NotRequested => {}
        }

        let user_fut = ctx
            .client()
            .get_respektive_user(self.user.user_id, self.user.mode);

        match user_fut.await {
            Ok(Some(user)) => Some(*self.score_rank.insert(user.rank)),
            Ok(None) => {
                self.score_rank = Availability::Errored;

                None
            }
            Err(err) => {
                warn!("{}", err.wrap_err("Failed to get respektive user"));
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

        self.user
            .statistics
            .as_ref()
            .map(|stats| bonus_pp.calculate(stats))
    }

    pub(crate) async fn top100stats(&mut self, ctx: &Context) -> Option<&Top100Stats> {
        if let Some(ref stats) = self.top100stats {
            return Some(stats);
        }

        let scores = self.get_scores(ctx).await?;
        let stats = Top100Stats::new(ctx, scores).await;

        Some(self.top100stats.insert(stats))
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

                let mut names = match ctx.psql().get_names_by_ids(&ids).await {
                    Ok(names) => names,
                    Err(err) => {
                        warn!("{:?}", err.wrap_err("Failed to get mapper names"));

                        HashMap::default()
                    }
                };

                if names.len() != ids.len() {
                    for (id, _) in entries.iter() {
                        if names.contains_key(id) {
                            continue;
                        }

                        let user = match ctx.osu().user(*id).mode(self.user.mode).await {
                            Ok(user) => user,
                            Err(err) => {
                                let err = Report::new(err).wrap_err("Failed to get user");
                                warn!("{err:?}");

                                continue;
                            }
                        };

                        let upsert_fut = ctx.psql().upsert_osu_user(&user, self.user.mode);

                        if let Err(err) = upsert_fut.await {
                            warn!("{:?}", err.wrap_err("Failed to upsert user"));
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
        let user_id = self.user.user_id;
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

        let user_args = UserArgs::new(&self.user.username, self.user.mode);
        let score_args = ScoreArgs::top(100);

        match get_scores(ctx, &user_args, &score_args).await {
            #[allow(unused_mut)]
            Ok(mut scores) => {
                #[cfg(feature = "osutracking")]
                crate::tracking::process_osu_tracking(ctx, &mut scores, Some(&self.user)).await;

                Some(self.scores.insert(scores))
            }
            Err(err) => {
                let err = Report::new(err).wrap_err("Failed to get top scores");
                warn!("{err:?}");
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
    pub len: MinMaxAvg<u32>,
}

impl Top100Stats {
    pub async fn new(ctx: &Context, scores: &[Score]) -> Self {
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

        let mut missing_pp = 0;
        let mut missing_map = 0;

        for score in scores {
            this.acc.add(score.accuracy);
            this.combo.add(score.max_combo);
            this.misses.add(score.statistics.count_miss);

            if let Some(pp) = score.pp {
                this.pp.add(pp);
            } else {
                missing_pp += 1;
            }

            if let Some(ref map) = score.map {
                this.len.add(map.seconds_drain);

                let diff_mods: GameMods = GameMods::HardRock
                    | GameMods::Easy
                    | GameMods::DoubleTime
                    | GameMods::HalfTime
                    | GameMods::Flashlight;

                if !score.mods.intersects(diff_mods) {
                    this.stars.add(map.stars as f64);
                    this.ar.add(map.ar as f64);
                    this.cs.add(map.cs as f64);
                    this.hp.add(map.hp as f64);
                    this.od.add(map.od as f64);
                    this.bpm.add(map.bpm);
                } else {
                    let calc = match PpCalculator::new(ctx, map.map_id).await {
                        Ok(calc) => calc,
                        Err(err) => {
                            warn!("{:?}", err.wrap_err("Failed to get pp calculator"));

                            continue;
                        }
                    };

                    let mut prepared = calc.score(score);
                    this.stars.add(prepared.stars());

                    let map_attrs = prepared
                        .map()
                        .attributes()
                        .mods(score.mods.bits())
                        .converted(map.convert)
                        .build();

                    this.ar.add(map_attrs.ar);
                    this.cs.add(map_attrs.cs);
                    this.hp.add(map_attrs.hp);
                    this.od.add(map_attrs.od);
                    this.bpm.add(map.bpm * map_attrs.clock_rate as f32);
                }
            } else {
                missing_map += 1;
            }
        }

        if missing_pp > 0 {
            warn!("Missing {missing_pp} pp values in top scores");
        }

        if missing_map > 0 {
            warn!("Missing {missing_map} maps in top scores");
        }

        this
    }
}

pub struct Top100Mods {
    pub percent_mods: Vec<(GameMods, u8)>,
    pub percent_mod_comps: Vec<(GameMods, u8)>,
    pub pp_mod_comps: Vec<(GameMods, f32)>,
}

impl Top100Mods {
    fn new(scores: &[Score]) -> Self {
        let mut percent_mods = HashMap::with_hasher(IntHasher);
        let mut percent_mod_comps = HashMap::with_hasher(IntHasher);
        let mut pp_mod_comps = HashMap::<_, f32, _>::with_hasher(IntHasher);

        for score in scores {
            *percent_mod_comps.entry(score.mods).or_default() += 1;

            if let Some(weight) = score.weight {
                *pp_mod_comps.entry(score.mods).or_default() += weight.pp;
            }

            for m in score.mods {
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
