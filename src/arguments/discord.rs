use super::Args;
use crate::util::matcher;

use std::str::FromStr;
use twilight::model::id::{ChannelId, MessageId, RoleId};

pub struct RoleAssignArgs {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    pub role_id: RoleId,
}

impl RoleAssignArgs {
    pub fn new(mut args: Args) -> Result<Self, &'static str> {
        let channel_id = args
            .next()
            .and_then(|arg| matcher::get_mention_channel(arg))
            .map(ChannelId);
        if channel_id.is_none() {
            return Err("Could not parse channel. Make sure your \
                        first argument is either a channel mention \
                        or a channel id.");
        }
        let message_id = args
            .next()
            .and_then(|arg| u64::from_str(arg).ok())
            .map(MessageId);
        if message_id.is_none() {
            return Err("Could not parse message. Make sure your \
                        second argument is a message id.");
        }
        let role_id = args
            .next()
            .and_then(|arg| matcher::get_mention_role(arg))
            .map(RoleId);
        if role_id.is_none() {
            return Err("Could not parse role. Make sure your \
                        third argument is either a role mention \
                        or a role id.");
        }
        Ok(Self {
            channel_id: channel_id.unwrap(),
            message_id: message_id.unwrap(),
            role_id: role_id.unwrap(),
        })
    }
}
