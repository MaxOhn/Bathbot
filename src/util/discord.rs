use crate::{util::globals::MSG_MEMORY, ResponseOwner};

use regex::Regex;
use serenity::{
    cache::CacheRwLock,
    model::{
        channel::{EmbedField, Message},
        guild::Member,
        id::{ChannelId, MessageId, UserId},
    },
    prelude::{Context, RwLock, ShareMap},
};
use std::{str::FromStr, sync::Arc};

pub fn map_id_from_history(msgs: Vec<Message>, cache: CacheRwLock) -> Option<u32> {
    let url_regex = Regex::new(r".*/([0-9]{1,9})").unwrap();
    let field_regex = Regex::new(r".*\{(\d+/){2,}\d+}.*").unwrap();
    let check_field = |url: &str, field: &EmbedField| {
        if field_regex.is_match(&field.value) {
            let result = url_regex
                .captures(url)
                .and_then(|caps| caps.get(1))
                .and_then(|cap| u32::from_str(cap.as_str()).ok());
            if result.is_some() {
                return result;
            }
        }
        None
    };
    for msg in msgs {
        if !msg.is_own(&cache) {
            continue;
        }
        for embed in msg.embeds {
            if let Some(author) = embed.author {
                if let Some(url) = author.url {
                    if url.contains("/b/") {
                        let result = url_regex
                            .captures(&url)
                            .and_then(|caps| caps.get(1))
                            .and_then(|cap| u32::from_str(cap.as_str()).ok());
                        if result.is_some() {
                            return result;
                        }
                    }
                }
            }
            if embed.fields.is_empty() {
                continue;
            }
            if let Some(url) = embed.url {
                if let Some(field) = embed.fields.first() {
                    if let Some(id) = check_field(&url, &field) {
                        return Some(id);
                    }
                }
                if let Some(field) = embed.fields.get(5) {
                    if let Some(id) = check_field(&url, &field) {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

pub fn save_response_owner(msg_id: MessageId, author_id: UserId, data: Arc<RwLock<ShareMap>>) {
    let mut data = data.write();
    let (queue, owners) = data
        .get_mut::<ResponseOwner>()
        .expect("Could not get ResponseOwner");
    queue.push_front(msg_id);
    if queue.len() > MSG_MEMORY {
        let oldest = queue.pop_back().unwrap();
        owners.remove(&oldest);
    }
    owners.insert(msg_id, author_id);
}

pub fn get_member(ctx: &Context, channel_id: ChannelId, user_id: UserId) -> Option<Member> {
    channel_id
        .to_channel(ctx)
        .ok()
        .and_then(|channel| channel.guild())
        .and_then(|guild_channel| guild_channel.read().guild_id.member(ctx, user_id).ok())
}
