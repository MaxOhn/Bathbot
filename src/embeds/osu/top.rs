use std::fmt::{self, Write};

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods, Score, User};

use crate::{
    commands::osu::TopScoreOrder,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::osu,
    pp::PpCalculator,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::{round, with_comma_int},
        ScoreExt,
    },
};

const MAX_TITLE_LENGTH: usize = 40;
const MAX_VERSION_LENGTH: usize = 20;

pub struct TopEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl TopEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: impl Into<TopScoreOrder>,
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
        sort_by: TopScoreOrder,
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

            let mapset_opt = if let TopScoreOrder::RankedDate = sort_by {
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
                {grade} {pp} • {acc}% • {score}{appendix}\n[ {combo} ] • {hits} • {ago}",
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
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}

pub struct CondensedTopEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl CondensedTopEmbed {
    pub async fn new<'i, S>(user: &User, scores: S, ctx: &Context, pages: (usize, usize)) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(512);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let pp = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => {
                    calc.score(score);

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => calc.pp() as f32,
                    };

                    Some(pp)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    None
                }
            };

            let _ = writeln!(
                description,
                "**{idx}. {grade} [{truncated_title} [{truncated_version}]]({OSU_BASE}b/{id}) {mods}**\n\
                {hits} • {acc}% • {pp} • {combo}x • {truncated_score}",
                idx = idx + 1,
                truncated_title = truncate_text(&mapset.title, MAX_TITLE_LENGTH),
                truncated_version = truncate_text(&map.version, MAX_VERSION_LENGTH),
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(score.mode),
                acc = score.acc(score.mode),
                combo = truncate_int(score.max_combo),
                pp = format!("{pp:.2}PP", pp = pp.unwrap_or(0.0)),
                truncated_score = truncate_int(score.score),
                hits = score.hits_string(score.mode),
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
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}
fn truncate_text(title: &String, max_length: usize) -> String {
    let mut new_title: String = "".to_owned();

    if title.len() > max_length {
        let mut count = 0;
        let iter = title.split_ascii_whitespace();

        for word in iter {
            if count + word.len() < max_length {
                new_title.push_str(word);
                new_title.push_str(" ");
                count += word.len() + 1;
            } else if count + word.len() > max_length {
                new_title.push_str(word);
                new_title.push_str("...");

                break;
            }
        }
    } else {
        new_title = title.to_string();
    }

    new_title
}

fn truncate_int(score: u32) -> String {
    for (num, chr) in [(1_000_000_000, "B"), (1_000_000, "M"), (1000, "K")] {
        let (div, mut rem) = (score / num, score % num);

        if div > 0 {
            while rem >= 100 {
                rem /= 10
            }

            return format!("{div}.{rem}{chr}");
        }
    }

    format!("{score}")
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

impl_builder!(CondensedTopEmbed {
    author,
    description,
    footer,
    thumbnail,
});

pub struct OrderAppendix<'a> {
    sort_by: TopScoreOrder,
    map: &'a Beatmap,
    ranked_date: Option<DateTime<Utc>>,
    score: &'a Score,
    farm: &'a HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(
        sort_by: TopScoreOrder,
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
            TopScoreOrder::Farm => {
                let mapset_id = self.map.mapset_id;
                let count = self
                    .farm
                    .get(&mapset_id)
                    .map_or(0, |(entry, _)| entry.count);

                write!(f, " ~ `{}`", with_comma_int(count))
            }
            TopScoreOrder::Bpm => {
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
            TopScoreOrder::Length => {
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
            TopScoreOrder::RankedDate => {
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
