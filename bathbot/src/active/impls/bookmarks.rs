use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Write,
    mem,
    sync::Arc,
    time::Duration,
};

use bathbot_macros::PaginationBuilder;
use bathbot_psql::model::osu::MapBookmark;
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::SecToMinSec,
    fields,
    numbers::round,
    EmbedBuilder, FooterBuilder, IntHasher, MessageOrigin,
};
use eyre::{Report, Result, WrapErr};
use futures::future::BoxFuture;
use rosu_pp::{AnyPP, Beatmap, BeatmapExt};
use rosu_v2::prelude::{GameMode, Username};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    core::Context,
    manager::redis::{osu::UserArgs, RedisData},
    util::{interaction::InteractionComponent, osu::mode_emote, Authored, ComponentExt, Emote},
};

#[derive(PaginationBuilder)]
pub struct BookmarksPagination {
    #[pagination(per_page = 1)]
    bookmarks: Vec<MapBookmark>,
    origin: MessageOrigin,
    cached_entries: CachedBookmarkEntries,
    defer_next: bool,
    filtered_maps: Option<bool>,
    msg_owner: Id<UserMarker>,
    content: String,
    pages: Pages,
}

impl BookmarksPagination {
    async fn cached_entry<'a>(
        ctx: &Context,
        entries: &'a mut CachedBookmarkEntries,
        map: &MapBookmark,
    ) -> Result<&'a CachedBookmarkEntry> {
        let entry = match entries.entry(map.map_id) {
            Entry::Occupied(entry) => return Ok(entry.into_mut()),
            Entry::Vacant(entry) => entry,
        };

        let map_fut = ctx.osu_map().pp_map(map.map_id);
        let creator_fut = creator_name(ctx, map);
        let (map_res, gd_creator) = tokio::join!(map_fut, creator_fut);
        let pp_map = map_res.wrap_err("Failed to get pp map")?;

        Ok(entry.insert(CachedBookmarkEntry { pp_map, gd_creator }))
    }

    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let defer = mem::replace(&mut self.defer_next, false);

        if self.bookmarks.is_empty() {
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

            return Ok(BuildPage::new(embed, defer));
        }

        let map = &self.bookmarks[self.pages.index()];

        let CachedBookmarkEntry { pp_map, gd_creator } =
            Self::cached_entry(&ctx, &mut self.cached_entries, map).await?;

        let mut attributes = pp_map.stars().calculate();
        let stars = attributes.stars();
        let max_combo = attributes.max_combo();

        const ACCS: [f32; 4] = [95.0, 97.0, 99.0, 100.0];
        let mut pps = Vec::with_capacity(ACCS.len());

        for &acc in ACCS.iter() {
            let pp_result = AnyPP::new(pp_map)
                .attributes(attributes)
                .accuracy(acc as f64)
                .calculate();

            let pp = pp_result.pp();

            let pp_str = if pp > 100_000.0 {
                format!("{pp:.3e}")
            } else {
                round(pp as f32).to_string()
            };

            pps.push(pp_str);
            attributes = pp_result.into();
        }

        let mut pp_values = String::with_capacity(128);
        let mut lens = Vec::with_capacity(ACCS.len());

        pp_values.push_str("```\nAcc ");

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

        for (pp, len) in pps.iter().zip(&lens) {
            let _ = write!(pp_values, "|{pp:^len$}");
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

        let info_name = format!("{mode} Map info", mode = mode_emote(map.mode));

        #[cfg(not(feature = "server"))]
        let url = "https://www.google.com";

        #[cfg(feature = "server")]
        let url = &crate::core::BotConfig::get().server.public_url;

        let download_value = format!(
            "[osu!direct]({url}/osudirect/{mapset_id})\n\
            [Mapset]({OSU_BASE}d/{mapset_id})\n\
            [No Video]({OSU_BASE}d/{mapset_id}n)\n\
            [Beatconnect](https://beatconnect.io/b/{mapset_id})",
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
            Some(ref name) => (name.as_str(), map.mapper_id),
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
            :frame_photo: [Full background](https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg)",
            mapset_id = map.mapset_id
        );

        if map.mode == GameMode::Osu {
            let _ = write!(
                description,
                " :clapper: [Map preview](http://jmir.xyz/osu/preview.html#{map_id})",
                map_id = map.map_id
            );
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

    async fn handle_remove(
        &mut self,
        ctx: &Context,
        component: &InteractionComponent,
    ) -> ComponentResult {
        let owner = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if owner != self.msg_owner {
            return ComponentResult::Ignore;
        }

        if let Err(err) = component.defer(ctx).await {
            return ComponentResult::Err(Report::new(err).wrap_err("Failed to defer component"));
        }

        let idx = self.pages.index();
        let bookmark = self.bookmarks.remove(idx);

        if let Err(err) = ctx.bookmarks().remove(owner, bookmark.map_id).await {
            return ComponentResult::Err(err);
        }

        self.pages = Pages::new(1, self.bookmarks.len());
        self.pages.set_index(idx);
        self.defer_next = true;

        debug!(user = %self.msg_owner, map = bookmark.map_id, "Removed bookmarked map");

        ComponentResult::BuildPage
    }
}

impl IActiveMessage for BookmarksPagination {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page(ctx))
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
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.pages.index() == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_custom = Button {
            custom_id: Some("bookmarks_remove".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Remove".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.pages.index() == self.pages.last_index(),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.pages.index() == self.pages.last_index(),
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let components = vec![
            Component::Button(jump_start),
            Component::Button(single_step_back),
            Component::Button(jump_custom),
            Component::Button(single_step),
            Component::Button(jump_end),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        if component.data.custom_id == "bookmarks_remove" {
            Box::pin(self.handle_remove(ctx, component))
        } else {
            handle_pagination_component(ctx, component, self.msg_owner, false, &mut self.pages)
        }
    }

    fn until_timeout(&self) -> Option<Duration> {
        (!self.bookmarks.is_empty()).then_some(Duration::from_secs(60))
    }
}

async fn creator_name<'m>(ctx: &Context, map: &MapBookmark) -> Option<Username> {
    if map.mapper_id == map.creator_id {
        return None;
    }

    match ctx.osu_user().name(map.mapper_id).await {
        Ok(name @ Some(_)) => return name,
        Ok(None) => {}
        Err(err) => warn!("{err:?}"),
    }

    let args = UserArgs::user_id(map.mapper_id);

    match ctx.redis().osu_user(args).await {
        Ok(RedisData::Original(user)) => Some(user.username),
        Ok(RedisData::Archive(user)) => Some(user.username.as_str().into()),
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
