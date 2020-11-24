mod args;
mod discord;
mod name;
mod osu_id;
mod osu_snipe;
mod osu_stats;
mod rank;
mod simulate;
mod top;

pub use args::Args;
pub use discord::*;
pub use name::*;
pub use osu_id::*;
pub use osu_snipe::*;
pub use osu_stats::*;
pub use rank::*;
pub use simulate::*;
pub use top::*;

use crate::{
    util::{matcher, osu::ModSelection},
    Context,
};

use rosu::model::Grade;
use std::str::FromStr;

fn mods(args: &mut Vec<String>) -> Option<ModSelection> {
    for (i, arg) in args.iter().enumerate() {
        let mods = matcher::get_mods(arg);
        if mods.is_some() {
            args.remove(i);
            return mods;
        }
    }
    None
}

fn acc(args: &mut Vec<String>) -> Result<Option<f32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-a" || arg == "-acc") {
        args.remove(idx);
        match args.get(idx).map(|arg| f32::from_str(arg.as_str())) {
            Some(Ok(acc)) => {
                args.remove(idx);
                Ok(Some(acc))
            }
            Some(Err(_)) => Err("Could not parse given accuracy, \
                try a decimal number between 0 and 100"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn combo(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-c" || arg == "-combo") {
        args.remove(idx);
        match args.get(idx).map(|arg| u32::from_str(arg.as_str())) {
            Some(Ok(combo)) => {
                args.remove(idx);
                Ok(Some(combo))
            }
            Some(Err(_)) => Err("Could not parse given combo, \
                try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn grade(args: &mut Vec<String>) -> Result<Option<Grade>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-g" || arg == "-grade") {
        args.remove(idx);
        match args.get(idx).map(|arg| Grade::from_str(arg)) {
            Some(Ok(grade)) => {
                args.remove(idx);
                Ok(Some(grade))
            }
            Some(Err(_)) => Err("Could not parse given grade, try SS, S, A, B, C, or D"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn n300(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-300" || arg == "-n300") {
        args.remove(idx);
        match args.get(idx).map(|arg| u32::from_str(arg.as_str())) {
            Some(Ok(n300)) => {
                args.remove(idx);
                Ok(Some(n300))
            }
            Some(Err(_)) => Err("Could not parse given n300, try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn n100(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-100" || arg == "-n100") {
        args.remove(idx);
        match args.get(idx).map(|arg| u32::from_str(arg.as_str())) {
            Some(Ok(n100)) => {
                args.remove(idx);
                Ok(Some(n100))
            }
            Some(Err(_)) => Err("Could not parse given n100, try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn n50(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-50" || arg == "-n50") {
        args.remove(idx);
        match args.get(idx).map(|arg| u32::from_str(arg.as_str())) {
            Some(Ok(n50)) => {
                args.remove(idx);
                Ok(Some(n50))
            }
            Some(Err(_)) => Err("Could not parse given n50, try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn score(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-s" || arg == "-score") {
        args.remove(idx);
        match args.get(idx).map(|arg| u32::from_str(arg.as_str())) {
            Some(Ok(score)) => {
                args.remove(idx);
                Ok(Some(score))
            }
            Some(Err(_)) => Err("Could not parse given score, \
                try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn miss(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-x" || arg == "-m") {
        args.remove(idx);
        match args.get(idx).map(|arg| u32::from_str(arg.as_str())) {
            Some(Ok(misses)) => {
                args.remove(idx);
                Ok(Some(misses))
            }
            Some(Err(_)) => Err("Could not parse given amount of misses, \
                try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn keywords(args: &mut Vec<String>, keys: &[&str]) -> bool {
    if let Some(idx) = args.iter().position(|arg| keys.contains(&arg.as_str())) {
        args.remove(idx);
        return true;
    }
    false
}

pub fn try_link_name(ctx: &Context, msg: Option<&str>) -> Option<String> {
    msg.and_then(|arg| {
        matcher::get_mention_user(arg)
            .and_then(|id| ctx.get_link(id))
            .or_else(|| Some(arg.to_owned()))
    })
}
