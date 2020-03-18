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
        let mut map_id = None;
        let mut mods = None;
        for arg in args {
            let maybe_map_id = arguments::get_regex_id(&arg);
            let maybe_mods = maybe_map_id.map_or_else(|| arguments::parse_mods(&arg), |_| None);
            if map_id.is_none() && maybe_map_id.is_some() {
                map_id = maybe_map_id;
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
        let (name, map_id) = match args.pop() {
            Some(arg) => {
                let id = arguments::get_regex_id(&arg);
                if id.is_some() {
                    (args.pop(), id)
                } else {
                    (Some(arg), None)
                }
            }
            None => (None, None),
        };
        Self { name, map_id }
    }
}
