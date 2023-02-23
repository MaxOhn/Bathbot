use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::EmbedData;
use bathbot_model::rkyv_impls::UsernameWrapper;
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder,
};
use eyre::{Report, Result, WrapErr};
use rkyv::{with::DeserializeWith, Infallible};
use rosu_pp::{AnyPP, BeatmapExt};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods, Username};
use time::OffsetDateTime;
use twilight_model::{
    channel::embed::EmbedField,
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};

use crate::{
    commands::osu::CustomAttrs,
    core::Context,
    embeds::attachment,
    manager::redis::{osu::UserArgs, RedisData},
    pagination::Pages,
    util::osu::mode_emote,
};

#[derive(EmbedData)]
pub struct MapEmbed {
    title: String,
    url: String,
    description: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    image: String,
    timestamp: OffsetDateTime,
    fields: Vec<EmbedField>,
}

impl MapEmbed {
    pub async fn new(
        map: &Beatmap,
        mapset: &Beatmapset,
        mods: GameMods,
        attrs: &CustomAttrs,
        origin: MessageOrigin,
        ctx: &Context,
        pages: &Pages,
    ) -> Result<Self> {
        let mut title = String::with_capacity(32);

        if map.mode == GameMode::Mania {
            let _ = write!(title, "[{}K] ", map.cs as u32);
        }

        let _ = write!(
            title,
            "{} - {}",
            mapset.artist.as_str().cow_escape_markdown(),
            mapset.title.as_str().cow_escape_markdown()
        );

        #[cfg(not(feature = "server"))]
        let url = "";

        #[cfg(feature = "server")]
        let url = &crate::core::BotConfig::get().server.public_url;

        let download_value = format!(
            "[osu!direct]({url}/osudirect/{mapset_id})\n\
            [Mapset]({OSU_BASE}d/{mapset_id})\n\
            [No Video]({OSU_BASE}d/{mapset_id}n)\n\
            [Beatconnect](https://beatconnect.io/b/{mapset_id})",
            mapset_id = map.mapset_id,
        );

        let mut seconds_total = map.seconds_total;
        let mut seconds_drain = map.seconds_drain;
        let mut bpm = map.bpm;

        if mods.contains(GameMods::DoubleTime) {
            seconds_total = (seconds_total as f32 * 2.0 / 3.0) as u32;
            seconds_drain = (seconds_drain as f32 * 2.0 / 3.0) as u32;
            bpm *= 1.5;
        } else if mods.contains(GameMods::HalfTime) {
            seconds_total = (seconds_total as f32 * 4.0 / 3.0) as u32;
            seconds_drain = (seconds_drain as f32 * 4.0 / 3.0) as u32;
            bpm *= 0.75;
        }

        let mut info_value = String::with_capacity(128);
        let mut fields = Vec::with_capacity(3);

        let map_fut = ctx.osu_map().pp_map(map.map_id);
        let creator_fut = creator_name(ctx, map, mapset);
        let (map_res, gd_creator) = tokio::join!(map_fut, creator_fut);

        let mut rosu_map = map_res.wrap_err("Failed to get pp map")?;

        let mod_bits = mods.bits();

        if let Some(ar_) = attrs.ar {
            rosu_map.ar = ar_ as f32;
        }

        if let Some(cs_) = attrs.cs {
            rosu_map.cs = cs_ as f32;
        }

        if let Some(hp_) = attrs.hp {
            rosu_map.hp = hp_ as f32;
        }

        if let Some(od_) = attrs.od {
            rosu_map.od = od_ as f32;
        }

        let map_attributes = rosu_map.attributes().mods(mod_bits).build();

        let mut attributes = rosu_map.stars().mods(mod_bits).calculate();
        let stars = attributes.stars();
        const ACCS: [f32; 4] = [95.0, 97.0, 99.0, 100.0];
        let mut pps = Vec::with_capacity(ACCS.len());

        for &acc in ACCS.iter() {
            let pp_result = AnyPP::new(&rosu_map)
                .mods(mod_bits)
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

        if let Some(combo) = map.max_combo {
            let _ = write!(info_value, "Combo: `{combo}x`");
        }

        let _ = writeln!(info_value, " Stars: [`{stars:.2}★`]({origin} \"{stars}\")",);
        let _ = write!(info_value, "Length: `{}` ", SecToMinSec::new(seconds_total));

        if seconds_drain != seconds_total {
            let _ = write!(info_value, "(`{}`) ", SecToMinSec::new(seconds_drain));
        }

        let _ = write!(
            info_value,
            "BPM: `{}` Objects: `{}`\nCS: `{}` AR: `{}` OD: `{}` HP: `{}` Spinners: `{}`",
            round(bpm),
            map.count_circles + map.count_sliders + map.count_spinners,
            round(map_attributes.cs as f32),
            round(map_attributes.ar as f32),
            round(map_attributes.od as f32),
            round(map_attributes.hp as f32),
            map.count_spinners,
        );

        let mut info_name = format!(
            "{mode} __[{version}]__",
            mode = mode_emote(map.mode),
            version = map.version.as_str().cow_escape_markdown()
        );

        if !mods.is_empty() {
            let _ = write!(info_name, " +{mods}");
        }

        fields![fields {
            info_name, info_value, true;
            "Download", download_value, true;
        }];

        let mut field_name = format!(
            ":heart: {}  :play_pause: {}  | {:?}, {:?}",
            WithComma::new(mapset.favourite_count),
            WithComma::new(mapset.playcount),
            mapset.language.expect("no language in mapset"),
            mapset.genre.expect("no genre in mapset"),
        );

        if mapset.nsfw {
            field_name.push_str(" :underage: NSFW");
        }

        fields![fields { field_name, pp_values, false }];

        let (date_text, timestamp) = if let Some(ranked_date) = mapset.ranked_date {
            (format!("{:?}", map.status), ranked_date)
        } else {
            ("Last updated".to_owned(), map.last_updated)
        };

        let mut author_text = format!("Created by {}", mapset.creator_name);

        if let Some(gd_creator) = gd_creator {
            let _ = write!(author_text, " (guest difficulty by {gd_creator})");
        }

        let author_icon = mapset.creator.as_ref().map_or_else(
            || format!("{AVATAR_URL}{}", mapset.creator_id),
            |creator| creator.avatar_url.to_owned(),
        );

        let author = AuthorBuilder::new(author_text)
            .url(format!("{OSU_BASE}u/{}", mapset.creator_id))
            .icon_url(author_icon);

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Map {page} out of {pages} in the mapset, {date_text}");

        let footer = FooterBuilder::new(footer_text);

        let image = attachment("map_graph.png");

        let mut description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{mapset_id}.mp3) \
            :frame_photo: [Full background](https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg)",
            mapset_id = mapset.mapset_id
        );

        if map.mode == GameMode::Osu {
            let _ = write!(
                description,
                " :clapper: [Map preview](http://jmir.xyz/osu/preview.html#{map_id})",
                map_id = map.map_id
            );
        }

        Ok(Self {
            title,
            image,
            footer,
            fields,
            author,
            timestamp,
            description,
            url: map.url.to_owned(),
        })
    }
}

#[derive(Copy, Clone)]
pub struct MessageOrigin {
    guild: Option<Id<GuildMarker>>,
    channel: Id<ChannelMarker>,
}

impl MessageOrigin {
    pub fn new(guild: Option<Id<GuildMarker>>, channel: Id<ChannelMarker>) -> Self {
        Self { guild, channel }
    }
}

impl Display for MessageOrigin {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let Self { guild, channel } = self;

        match guild {
            Some(guild) => write!(f, "https://discord.com/channels/{guild}/{channel}/#"),
            None => write!(f, "https://discord.com/channels/@me/{channel}/#"),
        }
    }
}

async fn creator_name<'m>(
    ctx: &Context,
    map: &Beatmap,
    mapset: &'m Beatmapset,
) -> Option<Username> {
    if map.creator_id == mapset.creator_id {
        return None;
    }

    match ctx.osu_user().name(map.creator_id).await {
        Ok(name @ Some(_)) => return name,
        Ok(None) => {}
        Err(err) => warn!("{err:?}"),
    }

    let args = UserArgs::user_id(map.creator_id);

    match ctx.redis().osu_user(args).await {
        Ok(RedisData::Original(user)) => Some(user.username),
        Ok(RedisData::Archived(user)) => {
            Some(UsernameWrapper::deserialize_with(&user.username, &mut Infallible).unwrap())
        }
        Err(err) => {
            warn!("{:?}", Report::new(err).wrap_err("Failed to get user"));

            None
        }
    }
}
