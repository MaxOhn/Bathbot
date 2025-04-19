use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_util::{
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageOrigin,
    constants::{AVATAR_URL, OSU_BASE},
    datetime::SecToMinSec,
    fields,
    numbers::{WithComma, round},
};
use eyre::{Result, WrapErr};
use rosu_pp::Difficulty;
use rosu_v2::prelude::{
    BeatmapExtended, BeatmapsetExtended, GameMode, GameModsIntermode, Username,
};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::osu::CustomAttrs,
    core::Context,
    embeds::attachment,
    manager::redis::osu::UserArgs,
    util::{
        Emote,
        interaction::{InteractionComponent, InteractionModal},
    },
};

#[derive(PaginationBuilder)]
pub struct MapPagination {
    mapset: BeatmapsetExtended,
    #[pagination(per_page = 1)]
    maps: Box<[BeatmapExtended]>,
    mods: GameModsIntermode,
    attrs: CustomAttrs,
    origin: MessageOrigin,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MapPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let map = &self.maps[self.pages.index()];

        let mut title = String::with_capacity(32);

        if map.mode == GameMode::Mania {
            let _ = write!(title, "[{}K] ", map.cs as u32);
        }

        let _ = write!(
            title,
            "{} - {}",
            self.mapset.artist.as_str().cow_escape_markdown(),
            self.mapset.title.as_str().cow_escape_markdown()
        );

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

        let mut seconds_total = map.seconds_total;
        let mut seconds_drain = map.seconds_drain;
        let mut bpm = map.bpm as f64;

        let clock_rate = self.mods.legacy_clock_rate();
        seconds_total = (seconds_total as f64 / clock_rate) as u32;
        seconds_drain = (seconds_drain as f64 / clock_rate) as u32;
        bpm *= clock_rate;

        let mut info_value = String::with_capacity(128);
        let mut fields = Vec::with_capacity(3);

        let map_manager = Context::osu_map();
        let map_fut = map_manager.pp_map(map.map_id);
        let creator_fut = creator_name(map, &self.mapset);
        let (map_res, gd_creator) = tokio::join!(map_fut, creator_fut);

        let mut rosu_map = map_res.wrap_err("Failed to get pp map")?;

        if let Some(ar_) = self.attrs.ar {
            rosu_map.ar = ar_ as f32;
        }

        if let Some(cs_) = self.attrs.cs {
            rosu_map.cs = cs_ as f32;
        }

        if let Some(hp_) = self.attrs.hp {
            rosu_map.hp = hp_ as f32;
        }

        if let Some(od_) = self.attrs.od {
            rosu_map.od = od_ as f32;
        }

        let map_attrs = rosu_map
            .attributes()
            .mods(&self.mods)
            .clock_rate(clock_rate)
            .build();

        const ACCS: [f32; 4] = [95.0, 97.0, 99.0, 100.0];
        let mut pps = Vec::with_capacity(ACCS.len());

        let stars = if rosu_map.check_suspicion().is_ok() {
            let mut attrs = Difficulty::new()
                .mods(&self.mods)
                .clock_rate(clock_rate)
                .calculate(&rosu_map);

            for &acc in ACCS.iter() {
                let pp_result = attrs
                    .performance()
                    .mods(&self.mods)
                    .accuracy(acc as f64)
                    .clock_rate(clock_rate)
                    .calculate();

                let pp = pp_result.pp();

                let pp_str = if pp > 100_000.0 {
                    format!("{pp:.3e}")
                } else {
                    round(pp as f32).to_string()
                };

                pps.push(pp_str);
                attrs = pp_result.into();
            }

            attrs.stars()
        } else {
            for _ in ACCS.iter() {
                pps.push("0".to_owned());
            }

            0.0
        };

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

        if let Some(combo) = map.max_combo {
            let _ = write!(info_value, "Combo: `{combo}x`");
        }

        let _ = writeln!(
            info_value,
            " Stars: [`{stars:.2}★`]({origin} \"{stars}\")",
            origin = self.origin
        );

        let _ = write!(info_value, "Length: `{}` ", SecToMinSec::new(seconds_total));

        if seconds_drain != seconds_total {
            let _ = write!(info_value, "(`{}`) ", SecToMinSec::new(seconds_drain));
        }

        let _ = write!(
            info_value,
            "BPM: `{}` Objects: `{}`\nCS: `{}` AR: `{}` OD: `{}` HP: `{}` Spinners: `{}`",
            round(bpm as f32),
            map.count_circles + map.count_sliders + map.count_spinners,
            round(map_attrs.cs as f32),
            round(map_attrs.ar as f32),
            round(map_attrs.od as f32),
            round(map_attrs.hp as f32),
            map.count_spinners,
        );

        let mut info_name = format!(
            "{mode} __[{version}]__",
            mode = Emote::from(map.mode),
            version = map.version.as_str().cow_escape_markdown()
        );

        if !self.mods.is_empty() {
            let _ = write!(info_name, " +{}", self.mods);
        }

        fields![fields {
            info_name, info_value, true;
            "Download", download_value, true;
        }];

        let mut field_name = format!(
            ":heart: {}  :play_pause: {}  | {:?}, {:?}",
            WithComma::new(self.mapset.favourite_count),
            WithComma::new(self.mapset.playcount),
            self.mapset.language.expect("no language in mapset"),
            self.mapset.genre.expect("no genre in mapset"),
        );

        if self.mapset.nsfw {
            field_name.push_str(" :underage: NSFW");
        }

        fields![fields { field_name, pp_values, false }];

        let (date_text, timestamp) = if let Some(ranked_date) = self.mapset.ranked_date {
            (format!("{:?}", map.status), ranked_date)
        } else {
            ("Last updated".to_owned(), map.last_updated)
        };

        let mut author_text = format!("Created by {}", self.mapset.creator_name);

        if let Some(gd_creator) = gd_creator {
            let _ = write!(author_text, " (guest difficulty by {gd_creator})");
        }

        let author_icon = self.mapset.creator.as_ref().map_or_else(
            || format!("{AVATAR_URL}{}", self.mapset.creator_id),
            |creator| creator.avatar_url.to_owned(),
        );

        let author = AuthorBuilder::new(author_text)
            .url(format!("{OSU_BASE}u/{}", self.mapset.creator_id))
            .icon_url(author_icon);

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();
        let footer_text = format!("Map {page} out of {pages} in the mapset, {date_text}");

        let footer = FooterBuilder::new(footer_text);

        let image = attachment("map_graph.png");

        let mut description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{mapset_id}.mp3) \
            :frame_photo: [Full background](https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg)",
            mapset_id = self.mapset.mapset_id
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
            .author(author)
            .description(description)
            .fields(fields)
            .footer(footer)
            .image(image)
            .timestamp(timestamp)
            .title(title)
            .url(map.url.as_str());

        let build = BuildPage::new(embed, true).content(self.content.clone());

        Ok(build)
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

impl MapPagination {
    pub fn set_index(&mut self, index: usize) {
        self.pages.set_index(index);
    }
}

async fn creator_name(map: &BeatmapExtended, mapset: &BeatmapsetExtended) -> Option<Username> {
    if map.creator_id == mapset.creator_id {
        return None;
    }

    match Context::osu_user().name(map.creator_id).await {
        Ok(name @ Some(_)) => return name,
        Ok(None) => {}
        Err(err) => warn!("{err:?}"),
    }

    let args = UserArgs::user_id(map.creator_id, GameMode::Osu);

    match Context::redis().osu_user(args).await {
        Ok(user) => Some(user.username.as_str().into()),
        Err(err) => {
            warn!(?err, "Failed to get user");

            None
        }
    }
}
