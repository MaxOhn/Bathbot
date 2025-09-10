use std::{collections::BTreeMap, fmt::Write};

use bathbot_util::{
    Authored, CowUtils, EmbedBuilder, FooterBuilder,
    constants::OSU_BASE,
    numbers::{last_multiple, round},
};
use eyre::{Report, Result};
use rosu_v2::prelude::{BeatmapsetExtended, BeatmapsetSearchResult, GameMode, Genre, Language};
use twilight_model::{
    channel::message::{
        Component,
        component::{ActionRow, Button, ButtonStyle},
    },
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::osu::{Search, SearchOrder},
    core::Context,
    util::{ComponentExt, Emote, interaction::InteractionComponent},
};

pub struct MapSearchPagination {
    maps: BTreeMap<usize, BeatmapsetExtended>,
    search_result: BeatmapsetSearchResult,
    args: Search,
    msg_owner: Id<UserMarker>,
    pages: MapSearchPages,
}

impl IActiveMessage for MapSearchPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let should_request_more = self.defer();

        if should_request_more {
            let next_fut = self.search_result.get_next(Context::osu());

            if let Some(mut next_search_result) = next_fut.await.transpose()? {
                let idx = self.pages.index();

                let iter = next_search_result
                    .mapsets
                    .drain(..)
                    .enumerate()
                    .map(|(i, s)| (idx + i, s));

                self.maps.extend(iter);
                self.search_result = next_search_result;
            }
        }

        let mut title = "Mapset results".to_owned();
        let sort = self.args.sort.unwrap_or_default();

        let non_empty_args = self.args.query.is_some()
            || self.args.mode.is_some()
            || self.args.status.is_some()
            || self.args.genre.is_some()
            || self.args.language.is_some()
            || self.args.video == Some(true)
            || self.args.storyboard == Some(true)
            || self.args.nsfw == Some(false)
            || sort != SearchOrder::Relevance
            || self.args.reverse == Some(true);

        if non_empty_args {
            title.push_str(" for `");
            let mut pushed = false;

            if let Some(ref query) = self.args.query {
                title.push_str(query);
                pushed = true;
            }

            if let Some(mode) = self.args.mode.map(GameMode::from) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "mode={mode}");
                pushed = true;
            }

            if let Some(ref status) = self.args.status {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "status={status:?}");
                pushed = true;
            }

            if let Some(genre) = self.args.genre.map(Genre::from) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "genre={genre:?}");
                pushed = true;
            }

            if let Some(language) = self.args.language.map(Language::from) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "language={language:?}");
                pushed = true;
            }

            if self.args.video == Some(true) {
                if pushed {
                    title.push(' ');
                }

                title.push_str("video=true");
                pushed = true;
            }

            if self.args.storyboard == Some(true) {
                if pushed {
                    title.push(' ');
                }

                title.push_str("storyboard=true");
                pushed = true;
            }

            if self.args.nsfw == Some(false) {
                if pushed {
                    title.push(' ');
                }

                title.push_str("nsfw=false");
                pushed = true;
            }

            if self.args.sort != Some(SearchOrder::Relevance) || self.args.reverse == Some(true) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(
                    title,
                    "sort={:?} ({})",
                    sort,
                    if self.args.reverse == Some(true) {
                        "asc"
                    } else {
                        "desc"
                    }
                );
            }

            title.push('`');
        }

        if self.maps.is_empty() {
            let embed = EmbedBuilder::new()
                .description("No maps found for the query")
                .footer(FooterBuilder::new("Page 1/1"))
                .title(title);

            return Ok(BuildPage::new(embed, should_request_more));
        }

        let index = self.pages.index();
        let entries = self.maps.range(index..index + 10);
        let mut description = String::with_capacity(512);

        for (&i, mapset) in entries {
            let mut mode = String::with_capacity(4);
            let maps = mapset.maps.as_ref().unwrap();

            if maps.iter().any(|map| map.mode == GameMode::Osu) {
                mode.push_str("osu!");
            }

            if maps.iter().any(|map| map.mode == GameMode::Mania) {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("mania");
            }

            if maps.iter().any(|map| map.mode == GameMode::Taiko) {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("taiko");
            }

            if maps.iter().any(|map| map.mode == GameMode::Catch) {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("ctb");
            }

            let _ = writeln!(
                description,
                "**#{idx} [{artist} - {title}]({OSU_BASE}s/{set_id})** [{count} map{plural}]\n\
                Creator: [{creator}]({OSU_BASE}u/{creator_id}) ({status:?}) • BPM: {bpm} • Mode: {mode}",
                idx = i + 1,
                artist = mapset.artist.cow_escape_markdown(),
                title = mapset.title.cow_escape_markdown(),
                set_id = mapset.mapset_id,
                count = maps.len(),
                plural = if maps.len() != 1 { "s" } else { "" },
                creator = mapset.creator_name.cow_escape_markdown(),
                creator_id = mapset.creator_id,
                status = mapset.status,
                bpm = round(mapset.bpm),
            );
        }

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .title(title);

        Ok(BuildPage::new(embed, should_request_more))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        match component.data.custom_id.as_str() {
            "pagination_start" => {
                self.pages.set_index(0);

                if self.defer()
                    && let Err(err) = component.defer().await.map_err(Report::new) {
                        return ComponentResult::Err(err.wrap_err("Failed to defer component"));
                    }
            }
            "pagination_back" => {
                let new_index = self.pages.index().saturating_sub(self.pages.per_page());
                self.pages.set_index(new_index);

                if self.defer()
                    && let Err(err) = component.defer().await.map_err(Report::new) {
                        return ComponentResult::Err(err.wrap_err("Failed to defer component"));
                    }
            }
            "pagination_step" => {
                let new_index = self.pages.index() + self.pages.per_page();
                self.pages.set_index(new_index);

                if self.defer()
                    && let Err(err) = component.defer().await.map_err(Report::new) {
                        return ComponentResult::Err(err.wrap_err("Failed to defer component"));
                    }
            }
            "pagination_end" => {
                self.pages.set_index(self.pages.last_index());

                if self.defer()
                    && let Err(err) = component.defer().await.map_err(Report::new) {
                        return ComponentResult::Err(err.wrap_err("Failed to defer component"));
                    }
            }
            other => {
                warn!(name = %other, ?component, "Unknown map search pagination component");

                return ComponentResult::Ignore;
            }
        }

        ComponentResult::BuildPage
    }
}

impl MapSearchPagination {
    pub fn new(
        maps: BTreeMap<usize, BeatmapsetExtended>,
        search_result: BeatmapsetSearchResult,
        args: Search,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        let pages = MapSearchPages::new(search_result.total as usize);

        Self {
            maps,
            search_result,
            args,
            msg_owner,
            pages,
        }
    }

    fn available_entries_in_page(&self) -> usize {
        let pages = &self.pages;

        self.maps
            .range(pages.index()..pages.index() + pages.per_page())
            .count()
    }

    fn defer(&self) -> bool {
        self.available_entries_in_page() < self.pages.per_page()
    }
}

pub struct MapSearchPages {
    index: usize,
    last_index: usize,
    reached_end: bool,
}

impl MapSearchPages {
    const PER_PAGE: usize = 10;

    pub fn new(amount: usize) -> Self {
        Self {
            index: 0,
            last_index: last_multiple(Self::PER_PAGE, amount),
            reached_end: false, // TODO
        }
    }

    const fn index(&self) -> usize {
        self.index
    }

    const fn last_index(&self) -> usize {
        self.last_index
    }

    const fn per_page(&self) -> usize {
        Self::PER_PAGE
    }

    const fn curr_page(&self) -> usize {
        self.index() / self.per_page() + 1
    }

    fn last_page(&self) -> usize {
        self.last_index() / self.per_page() + 1
    }

    fn set_index(&mut self, new_index: usize) {
        self.index = self.last_index().min(new_index);
        self.reached_end |= self.index() == self.last_index();
    }

    fn components(&self) -> Vec<Component> {
        if self.last_index() == 0 {
            return Vec::new();
        }

        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: self.index() == 0,
            emoji: Some(Emote::JumpStart.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.index() == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.index() == self.last_index(),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: !self.reached_end || self.index() == self.last_index(),
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let components = vec![
            Component::Button(jump_start),
            Component::Button(single_step_back),
            Component::Button(single_step),
            Component::Button(jump_end),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }
}
