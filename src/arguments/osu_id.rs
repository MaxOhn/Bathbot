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
        let mut args = arguments::first_n(&mut args, 2);
        let match_id = if let Some(id) = args.next().and_then(|arg| arguments::get_regex_id(&arg)) {
            id
        } else {
            return Err("The first argument must be either a match \
                        id or the multiplayer link to a match"
                .to_string());
        };
        let warmups = args
            .next()
            .and_then(|num| usize::from_str(&num).ok())
            .unwrap_or(2);
        Ok(Self { match_id, warmups })
    }
}

pub enum ID {
    Map(u32),
    Set(u32),
}

impl ID {
    pub fn get(&self) -> u32 {
        match self {
            ID::Map(id) => *id,
            ID::Set(id) => *id,
        }
    }
}

pub struct MapModArgs {
    pub map_id: Option<ID>,
    pub mods: Option<(GameMods, ModSelection)>,
}

impl MapModArgs {
    pub fn new(mut args: Args) -> Self {
        let args = arguments::first_n(&mut args, 2);
        let mut map_id = None;
        let mut mods = None;
        for arg in args {
            let maybe_map_id = arguments::get_regex_id(&arg);
            let maybe_mods = maybe_map_id.map_or_else(|| arguments::parse_mods(&arg), |_| None);
            if let Some(maybe_map_id) = maybe_map_id {
                if map_id.is_none() {
                    let id = if arg.contains("/s/")
                        || (arg.contains("/beatmapsets/") && !arg.contains('#'))
                    {
                        ID::Set(maybe_map_id)
                    } else {
                        ID::Map(maybe_map_id)
                    };
                    map_id = Some(id);
                }
            } else if mods.is_none() && maybe_mods.is_some() {
                mods = maybe_mods;
            }
        }
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
        let (name, map_id) = match args.next_back() {
            Some(arg) => {
                let id = arguments::get_regex_id(&arg);
                if id.is_some() {
                    (args.next(), id)
                } else {
                    (Some(arg), None)
                }
            }
            None => (None, None),
        };
        Self { name, map_id }
    }
}
