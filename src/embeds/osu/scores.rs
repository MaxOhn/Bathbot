use std::fmt::Write;

use eyre::Report;
use rosu::model::Score as ScoreV1;
use rosu_pp::{Beatmap as Map, BeatmapExt};
use rosu_v2::prelude::{Beatmap, GameMode, Score, User};
use twilight_model::channel::embed::EmbedField;

use crate::{
    core::Context,
    embeds::{osu, Author, Footer},
    error::PpError,
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago_dynamic,
        numbers::with_comma_int,
        osu::{grade_completion_mods, prepare_beatmap_file},
        ScoreExt,
    },
    BotResult,
};

pub struct ScoresEmbed {
    description: &'static str,
    fields: Vec<EmbedField>,
    thumbnail: String,
    footer: Footer,
    author: Author,
    title: String,
    url: String,
}

impl ScoresEmbed {
    #[allow(clippy::too_many_arguments)]
    pub async fn new<'i, S>(
        user: &User,
        map: &Beatmap,
        scores: S,
        idx: usize,
        pinned: &[Score],
        personal: &[Score],
        global_idx: Option<(usize, usize)>,
        ctx: &Context,
    ) -> Self
    where
        S: Iterator<Item = &'i ScoreV1>,
    {
        let mut fields = Vec::new();

        let pp_map = match get_map(ctx, map.map_id).await {
            Ok(map) => Some(map),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to prepare map for pp calculation");
                warn!("{report:?}");

                None
            }
        };

        for (i, score) in scores.enumerate() {
            let (pp, max_pp, stars) = match pp_map {
                Some(ref map) => {
                    let mods = score.enabled_mods.bits();
                    let performance = map.pp().mods(mods).calculate();

                    let max_pp = performance.pp() as f32;
                    let stars = performance.stars() as f32;

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => {
                            let performance = map
                                .pp()
                                .attributes(performance)
                                .mods(mods)
                                .n300(score.count300 as usize)
                                .n100(score.count100 as usize)
                                .n50(score.count50 as usize)
                                .n_katu(score.count_katu as usize)
                                .score(score.score)
                                .combo(score.max_combo as usize)
                                .misses(score.count_miss as usize)
                                .calculate();

                            performance.pp() as f32
                        }
                    };

                    (Some(pp), Some(max_pp), stars)
                }
                None => (score.pp, None, 0.0),
            };

            let stars = osu::get_stars(stars);
            let pp = osu::get_pp(pp, max_pp);

            let mut name = format!(
                "**{idx}.** {grade}\t[{stars}]\t{score}\t({acc}%)",
                idx = idx + i + 1,
                grade = grade_completion_mods(score, map),
                score = with_comma_int(score.score),
                acc = score.acc(map.mode),
            );

            if let Some(score_id) = score.score_id {
                let mods = score.enabled_mods.bits();

                if pinned
                    .iter()
                    .any(|s| s.score_id == score_id && s.mods.bits() == mods)
                {
                    let _ = write!(name, " 📌");
                };
            }

            let mut value = format!(
                "{pp} {combo} {hits} {ago}",
                combo = osu::get_combo(score, map),
                hits = score.hits_string(map.mode),
                ago = how_long_ago_dynamic(&score.date)
            );

            let personal_idx = personal.iter().position(|s| s.created_at == score.date);

            if personal_idx.is_some() || matches!(global_idx, Some((n, _)) if n == i) {
                value.push_str("\n__**");

                if let Some(idx) = personal_idx {
                    let _ = write!(value, "Personal Best #{}", idx + 1);
                }

                if let Some((_, idx)) = global_idx.filter(|(idx, _)| *idx == i) {
                    if personal_idx.is_some() {
                        value.reserve(19);
                        value.push_str(" and ");
                    }

                    let _ = write!(value, "Global Top #{}", idx + 1);
                }

                value.push_str("**__");
            }

            fields.push(field!(name, value, false));
        }

        let (artist, title, creator_name, creator_id, status) = {
            let ms = map
                .mapset
                .as_ref()
                .expect("mapset neither in map nor in option");

            (
                &ms.artist,
                &ms.title,
                &ms.creator_name,
                ms.creator_id,
                ms.status,
            )
        };

        let footer = Footer::new(format!("{:?} map by {}", status, creator_name))
            .icon_url(format!("{}{}", AVATAR_URL, creator_id));

        let description = fields
            .is_empty()
            .then(|| "No scores found")
            .unwrap_or_default();

        let mut title_text = String::with_capacity(32);

        let _ = write!(title_text, "{artist} - {title} [{}]", map.version);

        if map.mode == GameMode::MNA {
            let _ = write!(title_text, "[{}K] ", map.cs as u32);
        }

        Self {
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id),
            title: title_text,
            url: format!("{OSU_BASE}b/{}", map.map_id),
            fields,
            author: author!(user),
        }
    }
}

impl_builder!(ScoresEmbed {
    author,
    description,
    fields,
    footer,
    thumbnail,
    title,
    url,
});

async fn get_map(ctx: &Context, map_id: u32) -> BotResult<Map> {
    let map_path = prepare_beatmap_file(ctx, map_id).await?;
    let map = Map::from_path(map_path).await.map_err(PpError::from)?;

    Ok(map)
}
