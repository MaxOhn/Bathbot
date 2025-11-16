use std::{
    collections::{HashMap, hash_map::Entry},
    convert::identity,
    fmt::Write,
    mem,
    time::Duration,
};

use bathbot_psql::model::{configs::ListSize, osu::MapBookmark};
use bathbot_util::{
    Authored, CowUtils, EmbedBuilder, FooterBuilder, IntHasher, MessageOrigin,
    constants::{AVATAR_URL, OSU_BASE},
    datetime::SecToMinSec,
    fields,
    numbers::round,
};
use eyre::{Report, Result, WrapErr};
use rosu_pp::{Beatmap, Difficulty, Performance, any::HitResultPriority};
use rosu_v2::prelude::{GameMode, Username};
use twilight_model::{
    channel::message::{
        Component,
        component::{ActionRow, Button, ButtonStyle},
    },
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        impls::top::MapFormat,
        pagination::{Pages, handle_pagination_component},
    },
    core::Context,
    manager::redis::osu::UserArgs,
    util::{ComponentExt, Emote, interaction::InteractionComponent},
};

pub struct BookmarksPagination {
    bookmarks: Vec<MapBookmark>,
    origin: MessageOrigin,
    cached_entries: CachedBookmarkEntries,
    defer_next: bool,
    filtered_maps: Option<bool>,
    confirm_remove: Option<bool>,
    msg_owner: Id<UserMarker>,
    content: String,
    pages: Pages,
}

impl BookmarksPagination {
    const PER_PAGE_CONDENSED: usize = 10;
    const PER_PAGE_DETAILED: usize = 5;
    const PER_PAGE_SINGLE: usize = 1;

    pub fn new(
        bookmarks: Vec<MapBookmark>,
        origin: MessageOrigin,
        filtered_maps: Option<bool>,
        content: String,
        msg_owner: Id<UserMarker>,
        list_size: ListSize,
    ) -> Self {
        let per_page = match list_size {
            ListSize::Single => Self::PER_PAGE_SINGLE,
            ListSize::Detailed => Self::PER_PAGE_DETAILED,
            ListSize::Condensed => Self::PER_PAGE_CONDENSED,
        };

        let pages = Pages::new(per_page, bookmarks.len());

        Self {
            bookmarks,
            origin,
            cached_entries: CachedBookmarkEntries::default(),
            defer_next: false,
            filtered_maps,
            confirm_remove: None,
            msg_owner,
            content,
            pages,
        }
    }

    async fn cached_entry<'a>(
        entries: &'a mut CachedBookmarkEntries,
        map: &MapBookmark,
    ) -> Result<&'a CachedBookmarkEntry> {
        let entry = match entries.entry(map.map_id) {
            Entry::Occupied(entry) => return Ok(entry.into_mut()),
            Entry::Vacant(entry) => entry,
        };

        let map_manager = Context::osu_map();
        let map_fut = map_manager.pp_map(map.map_id);
        let creator_fut = creator_name(map);
        let (map_res, gd_creator) = tokio::join!(map_fut, creator_fut);
        let pp_map = map_res.wrap_err("Failed to get pp map")?;

        Ok(entry.insert(CachedBookmarkEntry { pp_map, gd_creator }))
    }

    async fn handle_remove(&mut self, component: &InteractionComponent) -> ComponentResult {
        let owner = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if owner != self.msg_owner {
            return ComponentResult::Ignore;
        }

        if let Err(err) = component.defer().await {
            return ComponentResult::Err(Report::new(err).wrap_err("Failed to defer component"));
        }

        let idx = self.pages.index();
        let bookmark = self.bookmarks.remove(idx);

        if let Err(err) = Context::bookmarks().remove(owner, bookmark.map_id).await {
            return ComponentResult::Err(err);
        }

        self.pages = Pages::new(1, self.bookmarks.len());
        self.pages.set_index(idx);
        self.defer_next = true;

        debug!(user = %self.msg_owner, map = bookmark.map_id, "Removed bookmarked map");

        ComponentResult::BuildPage
    }

    pub fn set_index(&mut self, index: usize) {
        self.pages.set_index(index);
    }

    fn build_empty_page(&mut self, defer: bool) -> Result<BuildPage> {
        let mut description = if self.filtered_maps.unwrap_or(false) {
            "No bookmarked maps match your criteria. \n\
            You can bookmark more maps by:\n"
                .to_owned()
        } else {
            "Looks like you haven't bookmarked any maps. \n\
            You can do so by:\n"
                .to_owned()
        };

        description.push_str(
            "1. Rightclicking a bot message that contains a single map\n\
            2. Click on `Apps`\n\
            3. Click on `Bookmark map`",
        );

        let embed = EmbedBuilder::new().description(description);

        Ok(BuildPage::new(embed, defer))
    }

    async fn build_page_condensed(&mut self) -> Result<BuildPage> {
        let defer = mem::replace(&mut self.defer_next, false);

        if self.bookmarks.is_empty() {
            return self.build_empty_page(defer);
        }

        let start = self.pages.index();
        let end = start + self.pages.per_page();

        let maps = &self.bookmarks[start..self.bookmarks.len().min(end)];
        let mut description = String::with_capacity(1024);

        for (i, map) in maps.iter().enumerate() {
            let entry_fut = Self::cached_entry(&mut self.cached_entries, map);

            let CachedBookmarkEntry {
                pp_map,
                gd_creator: _,
            } = match entry_fut.await {
                Ok(entry) => entry,
                Err(err) => {
                    warn!(?err, "Failed to prepare cached entry");

                    continue;
                }
            };

            let mut stars = 0.0;
            let mut max_pp = 0.0;

            let attrs_opt = Context::pp_parsed(pp_map, map.mode)
                .lazer(true)
                .performance()
                .await;

            if let Some(attrs) = attrs_opt {
                stars = attrs.stars() as f32;
                max_pp = attrs.pp() as f32;
            }

            let _ = writeln!(
                description,
                "**#{idx} [{map}]({OSU_BASE}b/{map_id})** [{stars} ★]\n\
                {mode} **{pp}pp** `{len}` {bpm_emote} {bpm} \
                `CS: {cs} AR: {ar} OD: {od} HP: {hp}`",
                idx = i + start + 1,
                map = MapFormat::new(&map.artist, &map.title, &map.version),
                map_id = map.map_id,
                stars = round(stars),
                mode = Emote::from(map.mode),
                pp = round(max_pp),
                len = SecToMinSec::new(map.seconds_total),
                bpm_emote = Emote::Bpm,
                bpm = round(map.bpm),
                cs = round(map.cs),
                ar = round(map.ar),
                od = round(map.od),
                hp = round(map.hp),
            );
        }

        let footer_text = format!(
            "Page {page}/{pages}",
            page = self.pages.curr_page(),
            pages = self.pages.last_page(),
        );

        let footer = FooterBuilder::new(footer_text);

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(footer)
            .title("Bookmarked maps");

        Ok(BuildPage::new(embed, defer).content(self.content.clone()))
    }

    async fn build_page_detailed(&mut self) -> Result<BuildPage> {
        let defer = mem::replace(&mut self.defer_next, false);

        if self.bookmarks.is_empty() {
            return self.build_empty_page(defer);
        }

        let start = self.pages.index();
        let end = start + self.pages.per_page();

        let maps = &self.bookmarks[start..self.bookmarks.len().min(end)];
        let mut description = String::with_capacity(1024);

        for (i, map) in maps.iter().enumerate() {
            let entry_fut = Self::cached_entry(&mut self.cached_entries, map);

            let CachedBookmarkEntry { pp_map, gd_creator } = match entry_fut.await {
                Ok(entry) => entry,
                Err(err) => {
                    warn!(?err, "Failed to prepare cached entry");

                    continue;
                }
            };

            let mut stars = 0.0;
            let mut max_combo = 0;
            let mut max_pp = 0.0;
            let mut pp_97 = 0.0;

            let attrs_opt = Context::pp_parsed(pp_map, map.mode)
                .lazer(true)
                .performance()
                .await;

            if let Some(attrs) = attrs_opt {
                stars = attrs.stars() as f32;
                max_combo = attrs.max_combo();
                max_pp = attrs.pp() as f32;

                pp_97 = Performance::new(attrs.difficulty_attributes())
                    .accuracy(97.0)
                    .hitresult_priority(HitResultPriority::Fastest)
                    .lazer(true)
                    .calculate()
                    .pp() as f32;
            }

            let _ = writeln!(
                description,
                "**#{idx} [{artist} - {title} [{version}]]({OSU_BASE}b/{map_id})** [{stars} ★]\n\
                **{pp_97}▸{pp}pp** for **97▸100%** • `{len}`• {max_combo}x • {bpm_emote} {bpm} \n\
                `CS: {cs} AR: {ar} OD: {od} HP: {hp}` • {mode} {status:?} map of {mapper}",
                idx = i + start + 1,
                artist = map.artist.cow_escape_markdown(),
                title = map.title.cow_escape_markdown(),
                version = map.version.cow_escape_markdown(),
                map_id = map.map_id,
                stars = round(stars),
                pp_97 = round(pp_97),
                pp = round(max_pp),
                len = SecToMinSec::new(map.seconds_total),
                bpm_emote = Emote::Bpm,
                bpm = round(map.bpm),
                cs = round(map.cs),
                ar = round(map.ar),
                od = round(map.od),
                hp = round(map.hp),
                mode = Emote::from(map.mode),
                status = map.status,
                mapper = match gd_creator {
                    Some(name) => name.as_str(),
                    None => map.creator_name.as_ref(),
                }
            );
        }

        let footer_text = format!(
            "Page {page}/{pages}",
            page = self.pages.curr_page(),
            pages = self.pages.last_page(),
        );

        let footer = FooterBuilder::new(footer_text);

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(footer)
            .title("Bookmarked maps");

        Ok(BuildPage::new(embed, defer).content(self.content.clone()))
    }

    async fn build_page_single(&mut self) -> Result<BuildPage> {
        let defer = mem::replace(&mut self.defer_next, false);

        if self.bookmarks.is_empty() {
            return self.build_empty_page(defer);
        }

        let map = &self.bookmarks[self.pages.index()];

        let CachedBookmarkEntry { pp_map, gd_creator } =
            Self::cached_entry(&mut self.cached_entries, map).await?;

        const ACCS: [f32; 4] = [95.0, 97.0, 99.0, 100.0];
        let mut pps = Vec::with_capacity(ACCS.len());

        let mut stars = 0.0;
        let mut max_combo = 0;

        if pp_map.check_suspicion().is_ok() {
            let attrs = Difficulty::new().calculate(pp_map);

            stars = attrs.stars();
            max_combo = attrs.max_combo();

            for &acc in ACCS.iter() {
                let pp_result = Performance::from(attrs.clone())
                    .accuracy(acc as f64)
                    .hitresult_priority(HitResultPriority::Fastest)
                    .calculate();

                let pp = pp_result.pp();

                let pp_str = if pp > 100_000.0 {
                    format!("{pp:.3e}")
                } else {
                    round(pp as f32).to_string()
                };

                pps.push(pp_str);
            }
        } else {
            for _ in ACCS.iter() {
                pps.push("0".to_owned());
            }
        }

        let mut pp_values = String::with_capacity(128);
        let mut lens = Vec::with_capacity(ACCS.len());

        pp_values.push_str("```ansi\nAcc ");

        for (pp, &acc) in pps.iter().zip(&ACCS) {
            let acc = acc.to_string() + "%";
            let len = pp.len().max(acc.len()) + 2;
            let _ = write!(pp_values, "|{acc:^len$}");
            lens.push(len);
        }

        pp_values.push_str("\n----");

        for len in lens.iter() {
            let _ = write!(pp_values, "+{:->len$}", "-");
        }

        pp_values.push_str("\n PP ");

        let bold = "\u{001b}[1m";
        let reset = "\u{001b}[0m";

        for (pp, len) in pps.iter().zip(&lens) {
            let _ = write!(pp_values, "|{bold}{pp:^len$}{reset}");
        }

        pp_values.push_str("\n```");

        let mut fields = Vec::with_capacity(3);

        let mut info_value = String::with_capacity(128);

        let _ = write!(info_value, "Combo: `{max_combo}x`");

        let _ = writeln!(
            info_value,
            " Stars: [`{stars:.2}★`]({origin} \"{stars}\")",
            origin = self.origin
        );

        let _ = write!(
            info_value,
            "Length: `{}` ",
            SecToMinSec::new(map.seconds_total)
        );

        if map.seconds_drain != map.seconds_total {
            let _ = write!(info_value, "(`{}`) ", SecToMinSec::new(map.seconds_drain));
        }

        let _ = write!(
            info_value,
            "BPM: `{}` Objects: `{}`\n\
            CS: `{}` AR: `{}` OD: `{}` HP: `{}` Spinners: `{}`",
            round(map.bpm),
            map.count_circles + map.count_sliders + map.count_spinners,
            round(map.cs),
            round(map.ar),
            round(map.od),
            round(map.hp),
            map.count_spinners,
        );

        let info_name = format!("{mode} Map info", mode = Emote::from(map.mode));

        #[cfg(not(feature = "server"))]
        let url = "https://www.google.com";

        #[cfg(feature = "server")]
        let url = &crate::core::BotConfig::get().server.public_url;

        let download_value = format!(
            "[osu!direct]({url}/osudirect/{mapset_id})\n\
            [catboy.best](https://catboy.best/d/{mapset_id})\n\
            [osu.direct](https://osu.direct/d/{mapset_id})\n\
            [nerinyan.moe](https://api.nerinyan.moe/d/{mapset_id})",
            mapset_id = map.mapset_id,
        );

        let field_name = format!("Language: {:?} • Genre: {:?}", map.language, map.genre);

        fields![fields {
            info_name, info_value, true;
            "Download", download_value, true;
            field_name, pp_values, false;
        }];

        let mut title = String::with_capacity(32);

        if map.mode == GameMode::Mania {
            let _ = write!(title, "[{}K] ", map.cs as u32);
        }

        let _ = write!(
            title,
            "{artist} - {title} [{version}]",
            artist = map.artist,
            title = map.title,
            version = map.version,
        );

        let (mapper_name, mapper_id) = match gd_creator {
            Some(name) => (name.as_str(), map.mapper_id),
            None => (map.creator_name.as_ref(), map.creator_id),
        };

        let footer_text = format!(
            "Page {page}/{pages} • {status:?} map of {mapper_name}",
            page = self.pages.curr_page(),
            pages = self.pages.last_page(),
            status = map.status,
        );

        let footer = FooterBuilder::new(footer_text).icon_url(format!("{AVATAR_URL}{mapper_id}"));

        let mut description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{mapset_id}.mp3) \
            :frame_photo: [Full background](https://catboy.best/preview/background/{mapset_id}/set)",
            mapset_id = map.mapset_id
        );

        match map.mode {
            GameMode::Osu => {
                let _ = write!(
                    description,
                    " :clapper: [Map preview](https://preview.tryz.id.vn/?b={map_id})",
                    map_id = map.map_id
                );
            }
            GameMode::Mania | GameMode::Taiko => {
                let _ = write!(
                    description,
                    " :clapper: [Map preview](https://osu-preview.jmir.xyz/preview#{map_id})",
                    map_id = map.map_id
                );
            }
            // Waiting on a preview website that supports catch
            GameMode::Catch => {}
        }

        let embed = EmbedBuilder::new()
            .description(description)
            .fields(fields)
            .footer(footer)
            .image(map.cover_url.as_ref())
            .title(title)
            .url(format!("{OSU_BASE}b/{}", map.map_id));

        Ok(BuildPage::new(embed, defer).content(self.content.clone()))
    }
}

impl IActiveMessage for BookmarksPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        match self.pages.per_page() {
            Self::PER_PAGE_SINGLE => self.build_page_single().await,
            Self::PER_PAGE_DETAILED => self.build_page_detailed().await,
            Self::PER_PAGE_CONDENSED => self.build_page_condensed().await,
            _ => unreachable!(),
        }
    }

    fn build_components(&self) -> Vec<Component> {
        if self.bookmarks.is_empty() {
            return Vec::new();
        }

        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: self.pages.index() == 0,
            emoji: Some(Emote::JumpStart.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.pages.index() == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let remove = if self.confirm_remove.is_some_and(identity) {
            Button {
                custom_id: Some("bookmarks_confirm_remove".to_owned()),
                disabled: false,
                emoji: None,
                label: Some("Confirm remove".to_owned()),
                style: ButtonStyle::Danger,
                url: None,
                sku_id: None,
            }
        } else {
            Button {
                custom_id: Some("bookmarks_remove".to_owned()),
                disabled: false,
                emoji: None,
                label: Some("Remove".to_owned()),
                style: ButtonStyle::Danger,
                url: None,
                sku_id: None,
            }
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.pages.index() == self.pages.last_index(),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.pages.index() == self.pages.last_index(),
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let mut components = Vec::with_capacity(5);

        components.push(Component::Button(jump_start));
        components.push(Component::Button(single_step_back));

        if self.pages.per_page() == 1 {
            components.push(Component::Button(remove));
        }

        components.push(Component::Button(single_step));
        components.push(Component::Button(jump_end));

        vec![Component::ActionRow(ActionRow { components })]
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        self.confirm_remove = Some(false);

        match component.data.custom_id.as_str() {
            "bookmarks_remove" => {
                self.confirm_remove = Some(true);

                ComponentResult::BuildPage
            }
            "bookmarks_confirm_remove" => self.handle_remove(component).await,
            _ => {
                self.defer_next = true;

                handle_pagination_component(component, self.msg_owner, true, &mut self.pages).await
            }
        }
    }

    fn until_timeout(&self) -> Option<Duration> {
        (!self.bookmarks.is_empty()).then_some(Duration::from_secs(60))
    }
}

async fn creator_name(map: &MapBookmark) -> Option<Username> {
    if map.mapper_id == map.creator_id {
        return None;
    }

    match Context::osu_user().name(map.mapper_id).await {
        Ok(name @ Some(_)) => return name,
        Ok(None) => {}
        Err(err) => warn!("{err:?}"),
    }

    let args = UserArgs::user_id(map.mapper_id, GameMode::Osu);

    match Context::redis().osu_user(args).await {
        Ok(user) => Some(user.username.as_str().into()),
        Err(err) => {
            warn!(?err, "Failed to get user");

            None
        }
    }
}

pub struct CachedBookmarkEntry {
    pp_map: Beatmap,
    gd_creator: Option<Username>,
}

pub type CachedBookmarkEntries = HashMap<u32, CachedBookmarkEntry, IntHasher>;
