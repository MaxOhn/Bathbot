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
    pub no_url: bool,
}

impl MarkovUserArgs {
    pub async fn new(mut args: Args, ctx: &Context, guild: GuildId) -> Result<Self, String> {
        if args.is_empty() {
            return Err("You need to provide a user as full discord tag, \
            as user id, or just as mention"
                .to_string());
        }
        let mut args = arguments::first_n(&mut args, 3);
        let mut arg = args.next().unwrap();
        let user = if let Ok(id) = u64::from_str(&arg) {
            UserId(id)
        } else if arg.starts_with("<@") && arg.ends_with('>') {
            arg.remove(0);
            arg.remove(0);
            if arg.contains('!') {
                arg.remove(0);
            }
            arg.pop();
            if let Ok(id) = u64::from_str(&arg) {
                UserId(id)
            } else {
                return Err("The first argument must be a user \
                    as full discord tag, as user id, or just as mention"
                    .to_string());
            }
        } else {
            let guild_arc = guild.to_guild_cached(&ctx.cache).await.unwrap();
            let guild = guild_arc.read().await;
            if let Some(member) = guild.member_named(&arg).await {
                member.user.id
            } else {
                return Err(format!("Could not get user from argument `{}`", arg));
            }
        };
        let mut no_url = None;
        let amount = match args.next() {
            Some(arg) => match usize::from_str(&arg) {
                Ok(num) => num.min(25),
                Err(_) => {
                    let found_url_arg =
                        ["-nourl", "-nourls", "-no-url", "-no-urls"].contains(&arg.as_str());
                    if found_url_arg {
                        no_url = Some(true);
                    }
                    10
                }
            },
            None => 10,
        };
        let no_url = no_url.unwrap_or_else(|| {
            args.next()
                .map(|arg| ["-nourl", "-nourls", "-no-url", "-no-urls"].contains(&arg.as_str()))
                .unwrap_or(false)
        });
        Ok(Self {
            user,
            amount,
            no_url,
        })
    }
}

pub struct MarkovChannelArgs {
    pub channel: Option<ChannelId>,
    pub amount: usize,
    pub no_url: bool,
}

impl MarkovChannelArgs {
    pub fn new(mut args: Args) -> Self {
        if args.is_empty() {
            return Self {
                channel: None,
                amount: 10,
                no_url: false,
            };
        }
        let mut args = arguments::first_n(&mut args, 2);
        let mut arg = args.next().unwrap();
        let channel = if let Ok(id) = u64::from_str(&arg) {
            Some(ChannelId(id))
        } else if arg.starts_with("<#") && arg.ends_with('>') {
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
        };
        let mut no_url = None;
        let amount = match args.next() {
            Some(arg) => match usize::from_str(&arg) {
                Ok(num) => num.min(25),
                Err(_) => {
                    let found_url_arg =
                        ["-nourl", "-nourls", "-no-url", "-no-urls"].contains(&arg.as_str());
                    if found_url_arg {
                        no_url = Some(true);
                    }
                    10
                }
            },
            None => 10,
        };
        let no_url = no_url.unwrap_or_else(|| {
            args.next()
                .map(|arg| ["-nourl", "-nourls", "-no-url", "-no-urls"].contains(&arg.as_str()))
                .unwrap_or(false)
        });
        Self {
            channel,
            amount,
            no_url,
        }
    }
}
