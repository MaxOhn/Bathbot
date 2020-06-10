use crate::arguments::{self, ModSelection};

use rosu::models::GameMods;
use serenity::{
    framework::standard::Args,
    model::{
        id::{GuildId, UserId},
        user::User,
    },
    prelude::Context,
};
use std::{collections::HashSet, iter::FromIterator, str::FromStr};

pub struct DiscordUserArgs {
    pub user: User,
}

impl DiscordUserArgs {
    pub async fn new(mut args: Args, ctx: &Context, guild: GuildId) -> Result<Self, String> {
        if args.is_empty() {
            return Err("You need to provide a user as full discord tag, \
                        as user id, or just as mention"
                .to_string());
        }
        let mut arg = args.single_quoted::<String>().unwrap();
        let user = if let Ok(id) = u64::from_str(&arg) {
            UserId(id)
                .to_user(ctx)
                .await
                .map_err(|_| "Error while retrieving user")?
        } else if arg.starts_with("<@") && arg.ends_with('>') {
            arg.remove(0);
            arg.remove(0);
            if arg.contains('!') {
                arg.remove(0);
            }
            arg.pop();
            if let Ok(id) = u64::from_str(&arg) {
                UserId(id)
                    .to_user(ctx)
                    .await
                    .map_err(|_| "Error while retrieving user")?
            } else {
                return Err("The first argument must be a user \
                as full discord tag, as user id, or just as mention"
                    .to_string());
            }
        } else {
            let guild = guild.to_guild_cached(&ctx.cache).await.unwrap();
            if let Some(member) = guild.member_named(&arg).await {
                member.user.clone()
            } else {
                return Err(format!("Could not get user from argument `{}`", arg));
            }
        };
        Ok(Self { user })
    }
}

pub struct NameArgs {
    pub name: Option<String>,
}

impl NameArgs {
    pub fn new(mut args: Args) -> Self {
        let mut args = arguments::first_n(&mut args, 1);
        Self { name: args.next() }
    }
}

pub struct MultNameArgs {
    pub names: HashSet<String>,
}

impl MultNameArgs {
    pub fn new(mut args: Args, n: usize) -> Self {
        let args = arguments::first_n(&mut args, n);
        Self {
            names: HashSet::from_iter(args),
        }
    }
}

pub struct NameFloatArgs {
    pub name: Option<String>,
    pub float: f32,
}

impl NameFloatArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 2);
        let float = args.next_back().and_then(|arg| f32::from_str(&arg).ok());
        if float.is_none() {
            return Err("You need to provide a decimal \
                        number as last argument"
                .to_string());
        }
        Ok(Self {
            name: args.next(),
            float: float.unwrap(),
        })
    }
}

pub struct NameIntArgs {
    pub name: Option<String>,
    pub number: Option<u32>,
}

impl NameIntArgs {
    pub fn new(mut args: Args) -> Self {
        let args = arguments::first_n(&mut args, 2);
        let mut name = None;
        let mut number = None;
        for arg in args {
            let res = u32::from_str(&arg).ok();
            if res.is_some() {
                number = res;
            } else {
                name = Some(arg);
            }
        }
        Self { name, number }
    }
}

pub struct NameModArgs {
    pub name: Option<String>,
    pub mods: Option<(GameMods, ModSelection)>,
}

impl NameModArgs {
    pub fn new(mut args: Args) -> Self {
        let args = arguments::first_n(&mut args, 2);
        let mut name = None;
        let mut mods = None;
        for arg in args {
            let res = arguments::parse_mods(&arg);
            if res.is_some() {
                mods = res;
            } else {
                name = Some(arg);
            }
        }
        Self { name, mods }
    }
}
