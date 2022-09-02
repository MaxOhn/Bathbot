use std::{collections::BTreeMap, fmt::Write};

use command_macros::EmbedData;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, User};

use crate::{
    core::Context,
    custom_client::SnipeScore,
    embeds::osu,
    pagination::Pages,
    pp::PpCalculator,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        hasher::SimpleBuildHasher,
        numbers::{round, with_comma_int},
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct PlayerSnipeListEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl PlayerSnipeListEmbed {
    pub async fn new(
        user: &User,
        scores: &BTreeMap<usize, SnipeScore>,
        maps: &HashMap<u32, Beatmap, SimpleBuildHasher>,
        total: usize,
        ctx: &Context,
        pages: &Pages,
    ) -> Self {
        if scores.is_empty() {
            return Self {
                author: author!(user),
                thumbnail: user.avatar_url.to_owned(),
                footer: FooterBuilder::new("Page 1/1 ~ Total #1 scores: 0"),
                description: "No scores were found".to_owned(),
            };
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let index = (page - 1) * 5;
        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);

        // TODO: update formatting
        for (idx, score) in entries {
            let map = maps.get(&score.map_id).expect("missing map");

            let max_pp = match PpCalculator::new(ctx, map.map_id).await {
                Ok(calc) => Some(calc.mods(score.mods).max_pp() as f32),
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    None
                }
            };

            let pp = osu::get_pp(score.pp, max_pp);
            let n300 = map.count_objects() - score.count_100 - score.count_50 - score.count_miss;

            let title = map
                .mapset
                .as_ref()
                .unwrap()
                .title
                .as_str()
                .cow_escape_markdown();

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {pp} ~ ({acc}%) ~ {score}\n{{{n300}/{n100}/{n50}/{nmiss}}} ~ {ago}",
                idx = idx + 1,
                version = map.version.as_str().cow_escape_markdown(),
                id = score.map_id,
                mods = osu::get_mods(score.mods),
                stars = score.stars,
                acc = round(score.accuracy),
                score = with_comma_int(score.score),
                n100 = score.count_100,
                n50 = score.count_50,
                nmiss = score.count_miss,
                ago = how_long_ago_dynamic(&score.score_date)
            );
        }

        let footer = FooterBuilder::new(format!("Page {page}/{pages} ~ Total scores: {total}"));

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}
