use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::Write,
};

use bathbot_macros::EmbedData;
use bathbot_model::{rosu_v2::user::User, SnipeScore};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder, IntHasher,
};
use eyre::Result;

use crate::{
    core::Context,
    manager::{redis::RedisData, OsuMap},
    pagination::Pages,
};

use super::{ModsFormatter, PpFormatter};

#[derive(EmbedData)]
pub struct PlayerSnipeListEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl PlayerSnipeListEmbed {
    pub async fn new(
        user: &RedisData<User>,
        scores: &BTreeMap<usize, SnipeScore>,
        maps: &HashMap<u32, OsuMap, IntHasher>,
        total: usize,
        ctx: &Context,
        pages: &Pages,
    ) -> Result<Self> {
        if scores.is_empty() {
            return Ok(Self {
                author: user.author_builder(),
                thumbnail: user.avatar_url().to_owned(),
                footer: FooterBuilder::new("Page 1/1 ~ Total #1 scores: 0"),
                description: "No scores were found".to_owned(),
            });
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let index = (page - 1) * 5;
        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);

        // TODO: update formatting
        for (idx, score) in entries {
            let map = maps.get(&score.map.map_id).expect("missing map");
            let mods = score.mods.as_ref().map(Cow::Borrowed).unwrap_or_default();
            let max_pp = ctx.pp(map).mods(mods.bits()).performance().await.pp() as f32;

            let _ = write!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {pp} ~ ({acc}%) ~ {score}\n{{{n300}/{n100}/{n50}/{nmiss}}}",
                idx = idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = score.map.map_id,
                mods = ModsFormatter::new(&mods),
                stars = score.stars,
                pp = PpFormatter::new(score.pp, Some(max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                n300 = score.count_300.unwrap_or(0),
                n100 = score.count_100.unwrap_or(0),
                n50 = score.count_50.unwrap_or(0),
                nmiss = score.count_miss.unwrap_or(0),
            );

            if let Some(ref date) = score.date_set {
                let _ = write!(description, " ~ {ago}", ago = HowLongAgoDynamic::new(date));
            }

            description.push('\n');
        }

        let footer = FooterBuilder::new(format!("Page {page}/{pages} ~ Total scores: {total}"));

        Ok(Self {
            author: user.author_builder(),
            description,
            footer,
            thumbnail: user.avatar_url().to_owned(),
        })
    }
}
