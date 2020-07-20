use super::{ArgResult, Args};
use crate::util::{matcher, osu::ModSelection};

use rosu::models::GameMods;
use std::str::FromStr;

pub struct MatchArgs {
    pub match_id: u32,
    pub warmups: usize,
}

impl MatchArgs {
    pub fn new(args: Args) -> ArgResult<Self> {
        let mut iter = args.iter();
        let match_id = match iter.next().and_then(|arg| matcher::get_osu_match_id(arg)) {
            Some(id) => id,
            None => {
                return Err("The first argument must be either a match \
                        id or the multiplayer link to a match"
                    .to_string())
            }
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
    pub fn new(args: Args) -> Self {
        let iter = args.iter();
        let mut map_id = None;
        let mut mods = None;
        for arg in iter {
            let maybe_map_id = matcher::get_osu_map_id(arg);
            let maybe_mods = maybe_map_id.map_or_else(|| matcher::get_mods(arg), |_| None);
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
    pub fn new(args: Args) -> Self {
        let mut iter = args.iter();
        let (name, map_id) = iter.next_back().map_or_else(
            || (None, None),
            |arg| {
                matcher::get_osu_map_id(arg).map_or_else(
                    || (Some(arg.to_owned()), None),
                    |id| (iter.next().map(|a| a.to_owned()), Some(id)),
                )
            },
        );
        Self { name, map_id }
    }
}
