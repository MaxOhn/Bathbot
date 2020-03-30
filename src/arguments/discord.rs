use crate::arguments;

use regex::Regex;
use serenity::{
    framework::standard::Args,
    model::id::{ChannelId, MessageId, RoleId},
};
use std::str::FromStr;

pub struct RoleAssignArgs {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    pub role_id: RoleId,
}

impl RoleAssignArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 3);
        let rgx = Regex::new(r"<#([0-9]*)>$").unwrap();
        let channel_id = args.next().and_then(|arg| parse(&arg, &rgx)).map(ChannelId);
        if channel_id.is_none() {
            return Err("Could not parse channel. Make sure your \
                        first argument is either a channel mention \
                        or a channel id."
                .to_string());
        }
        let message_id = args
            .next()
            .and_then(|arg| u64::from_str(&arg).ok())
            .map(MessageId);
        if message_id.is_none() {
            return Err("Could not parse message. Make sure your \
                        second argument is a message id."
                .to_string());
        }
        let rgx = Regex::new(r"<@&([0-9]*)>$").unwrap();
        let role_id = args.next().and_then(|arg| parse(&arg, &rgx)).map(RoleId);
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

pub struct RoleArgs {
    pub role_id: Option<RoleId>,
}

impl RoleArgs {
    pub fn new(mut args: Args) -> Self {
        let mut args = arguments::first_n(&mut args, 1);
        let rgx = Regex::new(r"<@&([0-9]*)>$").unwrap();
        let role_id = args.next().and_then(|arg| parse(&arg, &rgx)).map(RoleId);
        Self { role_id }
    }
}

fn parse(arg: &str, regex: &Regex) -> Option<u64> {
    u64::from_str(arg).ok().or_else(|| {
        regex
            .captures(&arg)
            .and_then(|caps| caps.get(1))
            .and_then(|cap| u64::from_str(cap.as_str()).ok())
    })
}
