use std::fmt::{self, Write};

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods, Score, User};

use crate::{
    commands::osu::TopOrder,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::{osu, Author, Footer},
    pp::PpCalculator,
    util::{
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::{round, with_comma_int},
        osu::ScoreOrder,
        ScoreExt,
    },
};

pub struct TopEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl TopEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: impl Into<TopOrder>,
        farm: &HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        Self::new_(user, scores, ctx, sort_by.into(), farm, pages).await
    }

    pub async fn new_<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: TopOrder,
        farm: &HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(512);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let (pp, max_pp, stars) = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => {
                    calc.score(score);

                    let stars = calc.stars();
                    let max_pp = calc.max_pp();

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => calc.pp() as f32,
                    };

                    (Some(pp), Some(max_pp as f32), stars as f32)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    (None, None, 0.0)
                }
            };

            let stars = osu::get_stars(stars);
            let pp = osu::get_pp(pp, max_pp);

            let mapset_opt = if let TopOrder::Other(ScoreOrder::RankedDate) = sort_by {
                let mapset_fut = ctx.psql().get_beatmapset::<Beatmapset>(mapset.mapset_id);

                match mapset_fut.await {
                    Ok(mapset) => Some(mapset),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("failed to get mapset");
                        warn!("{report:?}");

                        None
                    }
                }
            } else {
                None
            };

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ {acc}% ~ {score}{appendix}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx + 1,
                title = mapset.title,
                version = map.version,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(score.mode),
                acc = score.acc(score.mode),
                score = with_comma_int(score.score),
                appendix = OrderAppendix::new(sort_by, map, mapset_opt, score, farm),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(score.mode),
                ago = how_long_ago_dynamic(&score.created_at)
            );
        }

        description.pop();

        let footer_text = format!(
            "Page {}/{} | Mode: {}",
            pages.0,
            pages.1,
            mode_str(user.mode)
        );

        Self {
            author: author!(user),
            description,
            footer: Footer::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "osu!",
        GameMode::TKO => "Taiko",
        GameMode::CTB => "Catch",
        GameMode::MNA => "Mania",
    }
}

impl_builder!(TopEmbed {
    author,
    description,
    footer,
    thumbnail,
});

pub struct OrderAppendix<'a> {
    sort_by: TopOrder,
    map: &'a Beatmap,
    ranked_date: Option<DateTime<Utc>>,
    score: &'a Score,
    farm: &'a HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(
        sort_by: TopOrder,
        map: &'a Beatmap,
        mapset: Option<Beatmapset>,
        score: &'a Score,
        farm: &'a HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
    ) -> Self {
        let ranked_date = mapset.and_then(|mapset| mapset.ranked_date);

        Self {
            sort_by,
            map,
            ranked_date,
            score,
            farm,
        }
    }
}

impl fmt::Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.sort_by {
            TopOrder::Farm => {
                let mapset_id = self.map.mapset_id;
                let count = self
                    .farm
                    .get(&mapset_id)
                    .map_or(0, |(entry, _)| entry.count);

                write!(f, " ~ `{}`", with_comma_int(count))
            }
            TopOrder::Other(ScoreOrder::Bpm) => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                write!(f, " ~ `{}bpm`", round(self.map.bpm * clock_rate))
            }
            TopOrder::Other(ScoreOrder::Length) => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                let secs = (self.map.seconds_drain as f32 / clock_rate) as u32;

                write!(f, " ~ `{}:{:0>2}`", secs / 60, secs % 60)
            }
            TopOrder::Other(ScoreOrder::RankedDate) => {
                if let Some(date) = self.ranked_date {
                    write!(f, " ~ <t:{}:d>", date.timestamp())
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}
