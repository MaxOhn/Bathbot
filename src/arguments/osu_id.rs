use crate::arguments::{self, ModSelection};

use rosu::models::GameMods;
use serenity::framework::standard::Args;
use std::str::FromStr;

pub struct MatchArgs {
    pub match_id: u32,
    pub warmups: usize,
}

impl MatchArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let args = arguments::first_n(&mut args, 2);
        let mut iter = args.iter();
        let match_id = if let Some(id) = iter.next().and_then(|arg| arguments::get_regex_id(&arg)) {
            id
        } else {
            return Err("The first argument must be either a match \
                        id or the multiplayer link to a match"
                .to_string());
        };
        let warmups = iter
            .next()
            .and_then(|num| usize::from_str(&num).ok())
            .unwrap_or(2);
        Ok(Self { match_id, warmups })
    }
}

pub struct MapModArgs {
    pub map_id: Option<u32>,
    pub mods: Option<(GameMods, ModSelection)>,
}

impl MapModArgs {
    pub fn new(mut args: Args) -> Self {
        let args = arguments::first_n(&mut args, 2);
        let mut iter = args.into_iter();
        let map_id = iter.next().and_then(|arg| arguments::get_regex_id(&arg));
        let mods = iter.next().and_then(|arg| arguments::parse_mods(&arg));
        Self { map_id, mods }
    }
}

pub struct NameMapArgs {
    pub name: Option<String>,
    pub map_id: Option<u32>,
}

impl NameMapArgs {
    pub fn new(mut args: Args) -> Self {
        let mut args = arguments::first_n(&mut args, 2);
        let map_id = args.pop().and_then(|arg| arguments::get_regex_id(&arg));
        Self {
            name: args.pop(),
            map_id,
        }
    }
}
