use super::{ArgResult, Args};
use crate::util::matcher;

use regex::Regex;
use std::str::FromStr;
use twilight::model::id::{ChannelId, MessageId, RoleId};

pub struct RoleAssignArgs {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    pub role_id: RoleId,
}

impl RoleAssignArgs {
    pub fn new(args: Args) -> ArgResult<Self> {
        let mut iter = args.iter();
        let channel_id = iter
            .next()
            .and_then(|arg| matcher::get_mention_channel(arg))
            .map(ChannelId);
        if channel_id.is_none() {
            return Err("Could not parse channel. Make sure your \
                        first argument is either a channel mention \
                        or a channel id."
                .to_string());
        }
        let message_id = iter
            .next()
            .and_then(|arg| u64::from_str(arg).ok())
            .map(MessageId);
        if message_id.is_none() {
            return Err("Could not parse message. Make sure your \
                        second argument is a message id."
                .to_string());
        }
        let role_id = iter
            .next()
            .and_then(|arg| matcher::get_mention_role(arg))
            .map(RoleId);
        if role_id.is_none() {
            return Err("Could not parse role. Make sure your \
                        third argument is either a role mention \
                        or a role id."
                .to_string());
        }
        Ok(Self {
            channel_id: channel_id.unwrap(),
            message_id: message_id.unwrap(),
            role_id: role_id.unwrap(),
        })
    }
}
