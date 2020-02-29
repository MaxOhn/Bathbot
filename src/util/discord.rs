use crate::ResponseOwner;

use regex::Regex;
use serenity::{
    cache::CacheRwLock,
    model::{
        channel::{EmbedField, Message},
        id::{MessageId, UserId},
    },
    prelude::{RwLock, ShareMap},
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
    owners.insert(msg_id, author_id);
    queue.push_front(msg_id);
    if queue.len() > 1000 {
        let oldest = queue.pop_back().unwrap();
        owners.remove(&oldest);
    }
}
