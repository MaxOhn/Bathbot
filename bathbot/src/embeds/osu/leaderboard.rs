use std::{
    collections::hash_map::{Entry, HashMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::EmbedData;
use bathbot_model::ScraperScore;
use bathbot_util::{
    constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    numbers::WithComma,
    AuthorBuilder, CowUtils, FooterBuilder, IntHasher,
};
use rosu_pp::{BeatmapExt, DifficultyAttributes, ScoreState};
use rosu_v2::prelude::GameMode;

use crate::{
    core::Context,
    manager::{OsuMap, PpManager},
    pagination::Pages,
    util::{osu::grade_emote, Emote},
};

use super::PpFormatter;

type AttrMap = HashMap<u32, (DifficultyAttributes, f32), IntHasher>;

#[derive(EmbedData)]
pub struct LeaderboardEmbed {
    description: String,
    thumbnail: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
}

impl LeaderboardEmbed {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        author_name: Option<&str>,
        map: &OsuMap,
        stars: f32,
        attr_map: &mut AttrMap,
        scores: Option<&[ScraperScore]>,
        author_icon: &Option<String>,
        pages: &Pages,
        ctx: &Context,
    ) -> Self {
        let mut author_text = String::with_capacity(32);

        if map.mode() == GameMode::Mania {
            let _ = write!(author_text, "[{}K] ", map.cs() as u32);
        }

        let _ = write!(
            author_text,
            "{artist} - {title} [{version}] [{stars:.2}★]",
            artist = map.artist().cow_escape_markdown(),
            title = map.title().cow_escape_markdown(),
            version = map.version().cow_escape_markdown(),
        );

        let description = if let Some(scores) = scores {
            let mut description = String::with_capacity(256);
            let mut username = String::with_capacity(32);

            for (score, i) in scores.iter().zip(pages.index() + 1..) {
                let found_author = author_name == Some(score.username.as_str());
                username.clear();

                if found_author {
                    username.push_str("__");
                }

                let _ = write!(
                    username,
                    "[{name}]({OSU_BASE}users/{id})",
                    name = score.username.cow_escape_markdown(),
                    id = score.user_id
                );

                if found_author {
                    username.push_str("__");
                }

                let _ = writeln!(
                    description,
                    "**{i}.** {grade} **{username}**: {score} [ {combo} ] **+{mods}**\n\
                    - {pp} • {acc:.2}% • {miss}{ago}",
                    grade = grade_emote(score.grade),
                    score = WithComma::new(score.score),
                    combo = ComboFormatter::new(score, map),
                    mods = score.mods,
                    pp = pp_format(ctx, attr_map, score, map).await,
                    acc = score.accuracy,
                    miss = MissFormat(score.count_miss),
                    ago = HowLongAgoDynamic::new(&score.date),
                );
            }

            description
        } else {
            "No scores found".to_string()
        };

        let mut author =
            AuthorBuilder::new(author_text).url(format!("{OSU_BASE}b/{}", map.map_id()));

        if let Some(ref author_icon) = author_icon {
            author = author.icon_url(author_icon.to_owned());
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text = format!(
            "Page {page}/{pages} • {status:?} map",
            status = map.status(),
        );

        let footer_icon = format!("{AVATAR_URL}{creator_id}", creator_id = map.creator_id());
        let footer = FooterBuilder::new(footer_text).icon_url(footer_icon);

        Self {
            author,
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id()),
        }
    }
}

async fn pp_format(
    ctx: &Context,
    attr_map: &mut AttrMap,
    score: &ScraperScore,
    map: &OsuMap,
) -> PpFormatter {
    let mods = score.mods;

    match attr_map.entry(mods.bits()) {
        Entry::Occupied(entry) => {
            let (attrs, max_pp) = entry.get();

            let state = ScoreState {
                max_combo: score.max_combo as usize,
                n_geki: score.count_geki as usize,
                n_katu: score.count_katu as usize,
                n300: score.count300 as usize,
                n100: score.count100 as usize,
                n50: score.count50 as usize,
                n_misses: score.count_miss as usize,
            };

            let pp = map
                .pp_map
                .pp()
                .attributes(attrs.to_owned())
                .mode(PpManager::mode_conversion(score.mode))
                .mods(mods.bits())
                .state(state)
                .calculate()
                .pp() as f32;

            PpFormatter::new(Some(pp), Some(*max_pp))
        }
        Entry::Vacant(entry) => {
            let mut calc = ctx.pp(map).mode(score.mode).mods(mods);
            let attrs = calc.performance().await;
            let max_pp = attrs.pp() as f32;
            let pp = calc.score(score).performance().await.pp() as f32;
            entry.insert((attrs.into(), max_pp));

            PpFormatter::new(Some(pp), Some(max_pp))
        }
    }
}

struct ComboFormatter<'a> {
    score: &'a ScraperScore,
    map: &'a OsuMap,
}

impl<'a> ComboFormatter<'a> {
    fn new(score: &'a ScraperScore, map: &'a OsuMap) -> Self {
        Self { score, map }
    }
}

impl<'a> Display for ComboFormatter<'a> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "**{}x**", self.score.max_combo)?;

        if let Some(combo) = self.map.max_combo() {
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
    #[inline]
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
