use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{SnipeScore, SnipeScoreParams};
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder, IntHasher, ModsFormatter,
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{WithComma, round},
};
use eyre::{Result, WrapErr};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    core::Context,
    embeds::PpFormatter,
    manager::{OsuMap, redis::osu::CachedUser},
    util::{
        CachedUserExt, Emote,
        interaction::{InteractionComponent, InteractionModal},
    },
};

#[derive(PaginationBuilder)]
pub struct SnipePlayerListPagination {
    user: CachedUser,
    #[pagination(per_page = 5, len = "total")]
    scores: BTreeMap<usize, SnipeScore>,
    maps: HashMap<u32, OsuMap, IntHasher>,
    total: usize,
    params: SnipeScoreParams,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for SnipePlayerListPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;

        let count = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .count();

        if count < pages.per_page() && count < self.total - pages.index() {
            let huismetbenen_page = pages.index() / 50 + 1;
            self.params.page(huismetbenen_page as u32);

            // Get scores
            let scores = Context::client()
                .get_national_firsts(&self.params)
                .await
                .wrap_err("Failed to get national firsts")?;

            // Store scores in BTreeMap
            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| ((huismetbenen_page - 1) * 50 + i, s));

            self.scores.extend(iter);
        }

        // Get maps from DB
        let map_ids: HashMap<_, _, _> = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .filter_map(|(_, score)| {
                if self.maps.contains_key(&score.map_id) {
                    None
                } else {
                    Some((score.map_id as i32, None))
                }
            })
            .collect();

        if !map_ids.is_empty() {
            let new_maps = match Context::osu_map().maps(&map_ids).await {
                Ok(maps) => maps,
                Err(err) => {
                    warn!(?err, "Failed to get maps from database");

                    HashMap::default()
                }
            };

            self.maps.extend(new_maps);
        }

        if self.scores.is_empty() {
            let embed = EmbedBuilder::new()
                .author(self.user.author_builder(false))
                .description("No scores were found")
                .footer(FooterBuilder::new("Page 1/1 • Total #1 scores: 0"))
                .thumbnail(self.user.avatar_url.as_ref());

            return Ok(BuildPage::new(embed, true).content(self.content.clone()));
        }

        let entries = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page());

        let mut description = String::with_capacity(1024);

        for (idx, score) in entries {
            let map = self.maps.get(&score.map_id).expect("missing map");
            let mods = score.mods.as_ref().map(Cow::Borrowed).unwrap_or_default();

            let max_attrs = Context::pp(map)
                .mods(mods.clone().into_owned())
                .performance()
                .await;

            let mut max_pp = 0.0;
            let mut max_combo = 0;

            if let Some(max_attrs) = max_attrs {
                max_pp = max_attrs.pp() as f32;
                max_combo = max_attrs.max_combo();
            }

            let count_miss = score.count_miss.unwrap_or(0);

            let _ = write!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{map_id}) +{mods}**\n\
                {pp} • {acc}% • [{stars:.2}★]{miss}\n\
                [ {combo} ] • {score}",
                idx = idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = score.map_id,
                mods = ModsFormatter::new(&mods),
                pp = PpFormatter::new(score.pp, Some(max_pp)),
                stars = score.stars,
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, max_combo),
                miss = MissFormat(count_miss),
            );

            if let Some(ref date) = score.date_set {
                let _ = write!(description, " • {ago}", ago = HowLongAgoDynamic::new(date));
            }

            description.push('\n');
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} • Total scores: {}",
            self.total,
        ));

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(false))
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        handle_pagination_component(component, self.msg_owner, true, &mut self.pages).await
    }

    async fn handle_modal(&mut self, modal: &mut InteractionModal) -> Result<()> {
        handle_pagination_modal(modal, self.msg_owner, true, &mut self.pages).await
    }
}

struct ComboFormatter {
    combo: Option<u32>,
    max_combo: u32,
}

impl ComboFormatter {
    fn new(combo: Option<u32>, max_combo: u32) -> Self {
        Self { combo, max_combo }
    }
}

impl Display for ComboFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.combo {
            Some(combo) => write!(f, "**{combo}x**/")?,
            None => f.write_str("-/")?,
        }

        write!(f, "{}x", self.max_combo)
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(f, " • {miss}{emote}", miss = self.0, emote = Emote::Miss)
    }
}
