use crate::arguments;

use serenity::{
    framework::standard::Args,
    model::id::{ChannelId, GuildId, UserId},
    prelude::Context,
};
use std::str::FromStr;

pub struct MarkovUserArgs {
    pub user: UserId,
    pub amount: usize,
}

impl MarkovUserArgs {
    pub fn new(mut args: Args, ctx: &Context, guild: GuildId) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 2).into_iter();
        if args.len() == 0 {
            return Err("You need to provide a user as full discord tag, \
            as user id, or just as mention"
                .to_string());
        }
        let mut arg = args.next().unwrap();
        let user = if let Ok(id) = u64::from_str(&arg) {
            UserId(id)
        } else {
            if arg.starts_with("<@!") && arg.ends_with('>') {
                arg.remove(0);
                arg.remove(0);
                arg.remove(0);
                arg.pop();
                if let Ok(id) = u64::from_str(&arg) {
                    UserId(id)
                } else {
                    return Err("The first argument must be a user \
                    as full discord tag, as user id, or just as mention"
                        .to_string());
                }
            } else {
                let guild_arc = guild.to_guild_cached(&ctx.cache).unwrap();
                let guild = guild_arc.read();
                if let Some(member) = guild.member_named(&arg) {
                    member.user.read().id
                } else {
                    return Err(format!("Could not get user from argument `{}`", arg));
                }
            }
        };
        let amount = args
            .next()
            .and_then(|arg| usize::from_str(&arg).ok())
            .unwrap_or(10)
            .min(25);
        Ok(Self { user, amount })
    }
}

pub struct MarkovChannelArgs {
    pub channel: Option<ChannelId>,
    pub amount: usize,
}

impl MarkovChannelArgs {
    pub fn new(mut args: Args) -> Self {
        let mut args = arguments::first_n(&mut args, 2).into_iter();
        let mut arg = args.next().unwrap();
        let channel = if let Ok(id) = u64::from_str(&arg) {
            Some(ChannelId(id))
        } else {
            if arg.starts_with("<#") && arg.ends_with('>') {
                arg.remove(0);
                arg.remove(0);
                arg.pop();
                if let Ok(id) = u64::from_str(&arg) {
                    Some(ChannelId(id))
                } else {
                    None
                }
            } else {
                None
            }
        };
        let amount = args
            .next()
            .and_then(|arg| usize::from_str(&arg).ok())
            .unwrap_or(10)
            .min(25);
        Self { channel, amount }
    }
}
