use super::{ArgResult, Args};
use crate::{
    core::CachedUser,
    util::{matcher, osu::ModSelection},
    Context,
};

use rosu::models::GameMods;
use std::{collections::HashSet, iter::FromIterator, str::FromStr, sync::Arc};
use twilight::model::{
    id::{GuildId, UserId},
    user::User,
};

pub struct DiscordUserArgs {
    pub user: Arc<CachedUser>,
}

impl DiscordUserArgs {
    pub async fn new(mut args: Args<'_>, ctx: &Context, guild: GuildId) -> ArgResult<Self> {
        if args.is_empty() {
            return Err("You need to provide a user as full discord tag, \
                        as user id, or just as mention"
                .to_string());
        }
        let arg = args.single::<String>().unwrap();
        let user = match matcher::get_mention_user(&arg) {
            Some(id) => {
                let user = ctx.http.user(id).await;
                match user {
                    Ok(Some(user)) => Arc::new(CachedUser::from_user(&user)),
                    _ => return Err("Error while retrieving user".to_owned()),
                }
            }
            None => {
                let guild = ctx.cache.guilds.get(&guild);
                match guild.and_then(|guild| guild.member_named(&arg)) {
                    Some(member) => member.user.clone(),
                    None => return Err(format!("Could not get user from argument `{}`", arg)),
                }
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
        Self {
            name: args.single::<String>().ok(),
        }
    }
}

pub struct MultNameArgs {
    pub names: HashSet<String>,
}

impl MultNameArgs {
    pub fn new(args: Args, n: usize) -> Self {
        let iter = args.take(n).map(|arg| arg.to_owned());
        Self {
            names: HashSet::from_iter(iter),
        }
    }
}

pub struct NameFloatArgs {
    pub name: Option<String>,
    pub float: f32,
}

impl NameFloatArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take_all();
        let float = match args.next_back().and_then(|arg| f32::from_str(&arg).ok()) {
            Some(float) => float,
            None => return Err("You need to provide a decimal number as last argument"),
        };
        Ok(Self {
            name: args.next().map(|arg| arg.to_owned()),
            float,
        })
    }
}

pub struct NameIntArgs {
    pub name: Option<String>,
    pub number: Option<u32>,
}

impl NameIntArgs {
    pub fn new(args: Args) -> Self {
        let mut name = None;
        let mut number = None;
        for arg in args {
            let res = u32::from_str(arg).ok();
            if res.is_some() {
                number = res;
            } else {
                name = Some(arg.to_owned());
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
    pub fn new(args: Args) -> Self {
        let mut name = None;
        let mut mods = None;
        for arg in args {
            let res = matcher::get_mods(arg);
            if res.is_some() {
                mods = res;
            } else {
                name = Some(arg.to_owned());
            }
        }
        Self { name, mods }
    }
}
