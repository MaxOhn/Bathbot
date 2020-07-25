mod args;
mod discord;
mod name;
mod osu_id;
// mod osu_stats;
mod rank;
mod simulate;
// mod top;

pub use args::Args;
pub use discord::*;
pub use name::*;
pub use osu_id::*;
// pub use osu_stats::*;
pub use rank::*;
pub use simulate::*;
// pub use top::*;

use crate::util::{matcher, osu::ModSelection};

use regex::Regex;
use rosu::models::{GameMods, Grade};
use std::{borrow::Cow, convert::TryFrom, str::FromStr, vec::IntoIter};

type ArgResult<T> = Result<T, String>;

fn mods(args: &mut Vec<String>) -> Option<(GameMods, ModSelection)> {
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
        if let Some(num) = args.get(idx) {
            match f32::from_str(num) {
                Ok(acc) => {
                    args.remove(idx);
                    Ok(Some(acc))
                }
                Err(_) => {
                    Err("Could not parse given accuracy, try a decimal number between 0 and 100")
                }
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn combo(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-c" || arg == "-combo") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(combo) => {
                    args.remove(idx);
                    Ok(Some(combo))
                }
                Err(_) => Err("Could not parse given combo, try a non-negative integer"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn grade(args: &mut Vec<String>) -> Result<Option<Grade>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-g" || arg == "-grade") {
        args.remove(idx);
        if let Some(arg) = args.get(idx) {
            match Grade::try_from(arg.as_str()) {
                Ok(grade) => {
                    args.remove(idx);
                    Ok(Some(grade))
                }
                Err(_) => Err("Could not parse given grade, try SS, S, A, B, C, or D"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn n300(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-300" || arg == "-n300") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(n300) => {
                    args.remove(idx);
                    Ok(Some(n300))
                }
                Err(_) => Err("Could not parse given n300, try a non-negative integer"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn n100(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-100" || arg == "-n100") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(n100) => {
                    args.remove(idx);
                    Ok(Some(n100))
                }
                Err(_) => Err("Could not parse given n100, try a non-negative integer"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn n50(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-50" || arg == "-n50") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(n50) => {
                    args.remove(idx);
                    Ok(Some(n50))
                }
                Err(_) => Err("Could not parse given n50, try a non-negative integer"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn score(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-s" || arg == "-score") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(score) => {
                    args.remove(idx);
                    Ok(Some(score))
                }
                Err(_) => Err("Could not parse given score, try a non-negative integer"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn miss(args: &mut Vec<String>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| arg == "-x" || arg == "-m") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(misses) => {
                    args.remove(idx);
                    Ok(Some(misses))
                }
                Err(_) => Err("Could not parse given amount of misses, try a non-negative integer"),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn keywords(args: &mut Vec<String>, keys: &[&str]) -> bool {
    for (i, arg) in args.iter().enumerate() {
        if keys.contains(&arg.as_str()) {
            args.remove(i);
            return true;
        }
    }
    false
}
