mod discord;
mod name;
mod osu_id;
mod osu_stats;
mod rank;
mod simulate;
mod top;

pub use discord::*;
pub use name::*;
pub use osu_id::*;
pub use osu_stats::*;
pub use rank::*;
pub use simulate::*;
pub use top::*;

use regex::Regex;
use rosu::models::{GameMods, Grade};
use serenity::framework::standard::Args;
use std::{convert::TryFrom, str::FromStr, vec::IntoIter};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModSelection {
    None,
    Includes,
    Excludes,
    Exact,
}

pub fn get_regex_id(val: &str) -> Option<u32> {
    match u32::from_str(val) {
        Ok(id) => Some(id),
        Err(_) => {
            let regex = Regex::new(r".*/([0-9]{1,9})").unwrap();
            regex
                .captures(val)
                .and_then(|caps| caps.get(1).and_then(|id| u32::from_str(id.as_str()).ok()))
        }
    }
}

fn first_n(args: &mut Args, n: usize) -> IntoIter<String> {
    let mut v = Vec::with_capacity(n);
    while !args.is_empty() && v.len() < n {
        v.push(args.trimmed().single_quoted::<String>().unwrap());
    }
    v.into_iter()
}

fn parse_mods(arg: &str) -> Option<(GameMods, ModSelection)> {
    if arg.starts_with('+') {
        if arg.ends_with('!') {
            GameMods::try_from(&arg[1..arg.len() - 1])
                .ok()
                .map(|mods| (mods, ModSelection::Exact))
        } else {
            GameMods::try_from(&arg[1..])
                .ok()
                .map(|mods| (mods, ModSelection::Includes))
        }
    } else if arg.starts_with('-') && arg.ends_with('!') {
        GameMods::try_from(&arg[1..arg.len() - 1])
            .ok()
            .map(|mods| (mods, ModSelection::Excludes))
    } else {
        None
    }
}

fn mods(args: &mut Vec<String>) -> Option<(GameMods, ModSelection)> {
    for (i, arg) in args.iter().enumerate() {
        let mods = parse_mods(arg);
        if mods.is_some() {
            args.remove(i);
            return mods;
        }
    }
    None
}

fn acc(args: &mut Vec<String>) -> Result<Option<f32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-a" || arg == "-acc") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match f32::from_str(num) {
                Ok(acc) => {
                    args.remove(idx);
                    Ok(Some(acc))
                }
                Err(_) => Err(
                    "Could not parse given accuracy, try a decimal number between 0 and 100"
                        .to_string(),
                ),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn combo(args: &mut Vec<String>) -> Result<Option<u32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-c" || arg == "-combo") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(combo) => {
                    args.remove(idx);
                    Ok(Some(combo))
                }
                Err(_) => {
                    Err("Could not parse given combo, try a non-negative integer".to_string())
                }
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn grade(args: &mut Vec<String>) -> Result<Option<Grade>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-g" || arg == "-grade") {
        args.remove(idx);
        if let Some(arg) = args.get(idx) {
            match Grade::try_from(arg.as_str()) {
                Ok(grade) => {
                    args.remove(idx);
                    Ok(Some(grade))
                }
                Err(_) => Err("Could not parse given grade, try SS, S, A, B, C, or D".to_string()),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn n300(args: &mut Vec<String>) -> Result<Option<u32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-300" || arg == "-n300") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(n300) => {
                    args.remove(idx);
                    Ok(Some(n300))
                }
                Err(_) => Err("Could not parse given n300, try a non-negative integer".to_string()),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn n100(args: &mut Vec<String>) -> Result<Option<u32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-100" || arg == "-n100") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(n100) => {
                    args.remove(idx);
                    Ok(Some(n100))
                }
                Err(_) => Err("Could not parse given n100, try a non-negative integer".to_string()),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn n50(args: &mut Vec<String>) -> Result<Option<u32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-50" || arg == "-n50") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(n50) => {
                    args.remove(idx);
                    Ok(Some(n50))
                }
                Err(_) => Err("Could not parse given n50, try a non-negative integer".to_string()),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn score(args: &mut Vec<String>) -> Result<Option<u32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-s" || arg == "-score") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(score) => {
                    args.remove(idx);
                    Ok(Some(score))
                }
                Err(_) => {
                    Err("Could not parse given score, try a non-negative integer".to_string())
                }
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn miss(args: &mut Vec<String>) -> Result<Option<u32>, String> {
    if let Some(idx) = args.iter().position(|arg| arg == "-x" || arg == "-m") {
        args.remove(idx);
        if let Some(num) = args.get(idx) {
            match u32::from_str(num) {
                Ok(misses) => {
                    args.remove(idx);
                    Ok(Some(misses))
                }
                Err(_) => Err(
                    "Could not parse given amount of misses, try a non-negative integer"
                        .to_string(),
                ),
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
