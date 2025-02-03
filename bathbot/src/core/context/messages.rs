use std::time::Duration;

use bathbot_util::{matcher, osu::MapIdType};
use eyre::{Result, WrapErr};
use futures::StreamExt;
use time::OffsetDateTime;
use twilight_model::{
    channel::{message::Embed, Message},
    id::{marker::ChannelMarker, Id},
};

use crate::Context;

impl Context {
    pub async fn retrieve_channel_history(channel_id: Id<ChannelMarker>) -> Result<Vec<Message>> {
        Context::http()
            .channel_messages(channel_id)
            .limit(50)
            .await
            .wrap_err("Failed to request channel messages")?
            .models()
            .await
            .wrap_err("Failed to receive channel messages")
    }

    pub async fn find_map_id_in_msgs(msgs: &[Message], idx: usize) -> Option<MapIdType> {
        const SKIP_DELAY: Duration = Duration::from_millis(500);

        let now = OffsetDateTime::now_utc() - SKIP_DELAY;
        let secs = (now.unix_timestamp_nanos() / 1000) as i64;

        let iter = msgs
            .iter()
            .skip_while(|msg| msg.timestamp.as_micros() > secs);

        let stream = futures::stream::iter(iter)
            .filter_map(Self::find_map_id_in_msg)
            .skip(idx);

        tokio::pin!(stream);

        stream.next().await
    }

    pub async fn find_map_id_in_msg(msg: &Message) -> Option<MapIdType> {
        if let id @ Some(_) = Self::find_map_id_in_content(&msg.content) {
            id
        } else {
            Self::find_map_id_in_embeds(&msg.embeds).await
        }
    }

    fn find_map_id_in_content(content: &str) -> Option<MapIdType> {
        if content.chars().all(char::is_numeric) {
            return None;
        }

        matcher::get_osu_map_id(content)
            .map(MapIdType::Map)
            .or_else(|| matcher::get_osu_mapset_id(content).map(MapIdType::Set))
    }

    pub async fn find_map_id_in_embeds(embeds: &[Embed]) -> Option<MapIdType> {
        let opt = embeds.iter().find_map(|embed| {
            let url = embed
                .author
                .as_ref()
                .and_then(|author| author.url.as_deref());

            url.and_then(matcher::get_osu_map_id)
                .map(MapIdType::Map)
                .or_else(|| url.and_then(matcher::get_osu_mapset_id).map(MapIdType::Set))
                .or_else(|| {
                    embed
                        .url
                        .as_deref()
                        .and_then(matcher::get_osu_map_id)
                        .map(MapIdType::Map)
                })
                .or_else(|| {
                    embed
                        .url
                        .as_deref()
                        .and_then(matcher::get_osu_mapset_id)
                        .map(MapIdType::Set)
                })
                .or_else(|| {
                    embed
                        .description
                        .as_deref()
                        .and_then(matcher::get_single_osu_map_id)
                        .map(MapIdType::Map)
                })
        });

        if opt.is_some() {
            return opt;
        }

        for embed in embeds {
            // check the description for youtube video's & co
            if let Some(map_id) = embed
                .description
                .as_deref()
                .filter(|_| embed.kind == "video")
                .and_then(matcher::get_osu_map_id)
            {
                return Some(MapIdType::Map(map_id));
            }

            // if it's an ordr url, try to request the map id for it
            let video_url_opt = embed
                .url
                .as_ref()
                .and_then(|url| url.strip_prefix("https://link.issou.best/"));

            let Some(video_url) = video_url_opt else {
                continue;
            };

            let Some(ordr) = Context::ordr() else {
                continue;
            };

            let render_opt = ordr
                .client()
                .render_list()
                .link(video_url)
                .page_size(1)
                .page(1)
                .await
                .ok()
                .and_then(|mut list| list.renders.pop().filter(|_| list.renders.is_empty()));

            let Some(render) = render_opt else { continue };

            let Ok(versions) = Context::osu_map().versions_by_mapset(render.map_id).await else {
                continue;
            };

            let version_opt = versions
                .iter()
                .find(|entry| entry.version.as_str() == render.replay_difficulty.as_ref());

            if let Some(version) = version_opt {
                return Some(MapIdType::Map(version.map_id as u32));
            }
        }

        None
    }
}
