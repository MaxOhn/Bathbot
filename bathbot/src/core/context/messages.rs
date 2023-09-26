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
    pub async fn retrieve_channel_history(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> Result<Vec<Message>> {
        self.http
            .channel_messages(channel_id)
            .limit(50)
            .unwrap()
            .await
            .wrap_err("failed to request channel messages")?
            .models()
            .await
            .wrap_err("failed to deserialize channel messages")
    }

    pub async fn find_map_id_in_msgs(&self, msgs: &[Message], idx: usize) -> Option<MapIdType> {
        const SKIP_DELAY: Duration = Duration::from_millis(500);

        let now = OffsetDateTime::now_utc() - SKIP_DELAY;
        let secs = (now.unix_timestamp_nanos() / 1000) as i64;

        let iter = msgs
            .iter()
            .skip_while(|msg| msg.timestamp.as_micros() > secs);

        let stream = futures::stream::iter(iter)
            .filter_map(|msg| self.find_map_id_in_msg(msg))
            .skip(idx);

        tokio::pin!(stream);

        stream.next().await
    }

    pub async fn find_map_id_in_msg(&self, msg: &Message) -> Option<MapIdType> {
        if msg.content.chars().all(|c| c.is_numeric()) {
            return self.find_map_id_in_embeds(&msg.embeds).await;
        }

        let opt = matcher::get_osu_map_id(&msg.content)
            .map(MapIdType::Map)
            .or_else(|| matcher::get_osu_mapset_id(&msg.content).map(MapIdType::Set));

        match opt {
            id @ Some(_) => id,
            None => self.find_map_id_in_embeds(&msg.embeds).await,
        }
    }

    pub async fn find_map_id_in_embeds(&self, embeds: &[Embed]) -> Option<MapIdType> {
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

            let Some(ordr) = self.ordr() else { continue };

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
            let Ok(versions) = self.osu_map().versions_by_mapset(render.map_id).await else {
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
