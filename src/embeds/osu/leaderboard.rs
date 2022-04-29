use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use command_macros::EmbedData;
use hashbrown::{hash_map::Entry, HashMap};
use rosu_pp::{Beatmap as Map, BeatmapExt, DifficultyAttributes};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode};

use crate::{
    core::Context,
    custom_client::ScraperScore,
    error::PpError,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago_dynamic,
        numbers::with_comma_int,
        osu::prepare_beatmap_file,
        CowUtils, Emote, ScoreExt,
    },
    BotResult,
};

#[derive(EmbedData)]
pub struct LeaderboardEmbed {
    description: String,
    thumbnail: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
}

impl LeaderboardEmbed {
    #[allow(clippy::too_many_arguments)]
    pub async fn new<'i, S>(
        author_name: Option<&str>,
        map: &Beatmap,
        scores: Option<S>,
        author_icon: &Option<String>,
        idx: usize,
        ctx: &Context,
        pages: (usize, usize),
    ) -> BotResult<Self>
    where
        S: Iterator<Item = &'i ScraperScore>,
    {
        let Beatmapset {
            artist,
            title,
            creator_name,
            creator_id,
            ..
        } = map.mapset.as_ref().unwrap();

        let mut author_text = String::with_capacity(32);

        if map.mode == GameMode::MNA {
            let _ = write!(author_text, "[{}K] ", map.cs as u32);
        }

        let _ = write!(
            author_text,
            "{artist} - {title} [{}] [{:.2}★]",
            map.version, map.stars
        );

        let description = if let Some(scores) = scores {
            let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
            let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;

            let mut mod_map = HashMap::new();
            let mut description = String::with_capacity(256);
            let author_name = author_name.unwrap_or_default();
            let mut username = String::with_capacity(32);

            for (score, i) in scores.zip(idx + 1..) {
                let found_author = author_name == score.username;
                username.clear();

                if found_author {
                    username.push_str("__");
                }

                let _ = write!(
                    username,
                    "[{name}]({OSU_BASE}users/{id})",
                    name = score.username.cow_replace('_', "\\_"),
                    id = score.user_id
                );

                if found_author {
                    username.push_str("__");
                }

                let _ = writeln!(
                    description,
                    "**{i}.** {grade} **{username}**: {score} [ {combo} ] **+{mods}**\n\
                    - {pp} • {acc:.2}% • {miss}{ago}",
                    grade = score.grade_emote(map.mode),
                    score = with_comma_int(score.score),
                    combo = ComboFormatter::new(score, map),
                    mods = score.mods,
                    pp = get_pp(&mut mod_map, score, &rosu_map).await,
                    acc = score.accuracy,
                    miss = MissFormat(score.count_miss),
                    ago = how_long_ago_dynamic(&score.date),
                );
            }

            description
        } else {
            "No scores found".to_string()
        };

        let mut author = AuthorBuilder::new(author_text).url(format!("{OSU_BASE}b/{}", map.map_id));

        if let Some(ref author_icon) = author_icon {
            author = author.icon_url(author_icon.to_owned());
        }

        let footer_text = format!(
            "{:?} map by {creator_name} • Page {}/{}",
            map.status, pages.0, pages.1,
        );

        let footer = FooterBuilder::new(footer_text).icon_url(format!("{AVATAR_URL}{creator_id}"));

        Ok(Self {
            author,
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id),
        })
    }
}

async fn get_pp(
    mod_map: &mut HashMap<u32, (DifficultyAttributes, f32)>,
    score: &ScraperScore,
    map: &Map,
) -> PPFormatter {
    let bits = score.mods.bits();

    let (attrs, max_pp) = match mod_map.entry(bits) {
        Entry::Occupied(entry) => {
            let (attrs, max_pp) = entry.get();

            (attrs.to_owned(), *max_pp)
        }
        Entry::Vacant(entry) => {
            let attrs = map.max_pp(bits);
            let max_pp = attrs.pp() as f32;
            let (attrs, max_pp) = entry.insert((attrs.into(), max_pp));

            (attrs.to_owned(), *max_pp)
        }
    };

    let pp = map
        .pp()
        .attributes(attrs)
        .mods(score.mods.bits())
        .state(score.into())
        .calculate()
        .pp() as f32;

    PPFormatter(pp, max_pp)
}

struct PPFormatter(f32, f32);

impl Display for PPFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "**{:.2}**/{:.2}PP", self.0, self.1)
    }
}

struct ComboFormatter<'a> {
    score: &'a ScraperScore,
    map: &'a Beatmap,
}

impl<'a> ComboFormatter<'a> {
    fn new(score: &'a ScraperScore, map: &'a Beatmap) -> Self {
        Self { score, map }
    }
}

impl<'a> Display for ComboFormatter<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "**{}x**", self.score.max_combo)?;

        if let Some(combo) = self.map.max_combo {
            write!(f, "/{combo}x")
        } else {
            let mut ratio = self.score.count_geki as f32;

            if self.score.count300 > 0 {
                ratio /= self.score.count300 as f32
            }

            write!(f, " / {ratio:.2}")
        }
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(
            f,
            "{miss}{emote} ",
            miss = self.0,
            emote = Emote::Miss.text()
        )
    }
}
