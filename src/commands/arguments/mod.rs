#![allow(dead_code)]

use regex::Regex;
use rosu::models::GameMods;
use serenity::{
    framework::standard::Args,
    model::id::{ChannelId, RoleId},
};
use std::{convert::TryFrom, str::FromStr};

pub struct ArgParser {
    args: Args,
}

impl ArgParser {
    pub fn new(args: Args) -> Self {
        Self { args }
    }

    /// Search for `+mods` (included), `+mods!` (exact), or `-mods!` (excluded)
    pub fn get_mods(&mut self) -> Option<(GameMods, ModSelection)> {
        for arg in self.args.trimmed().iter::<String>() {
            if let Ok(arg) = arg {
                if arg.starts_with('+') {
                    if arg.ends_with('!') {
                        let mods = match GameMods::try_from(&arg[1..arg.len() - 1]) {
                            Ok(mods) => mods,
                            Err(_) => return None,
                        };
                        return Some((mods, ModSelection::Exact));
                    } else {
                        let mods = match GameMods::try_from(&arg[1..]) {
                            Ok(mods) => mods,
                            Err(_) => return None,
                        };
                        return Some((mods, ModSelection::Includes));
                    }
                } else if arg.starts_with('-') && arg.ends_with('!') {
                    let mods = match GameMods::try_from(&arg[1..arg.len() - 1]) {
                        Ok(mods) => mods,
                        Err(_) => return None,
                    };
                    return Some((mods, ModSelection::Excludes));
                }
            }
        }
        None
    }

    /// Search for `-c` or `-combo` and return the succeeding argument
    pub fn get_combo(&self) -> Option<String> {
        self.get_parameter(&["-c", "-combo"])
    }

    /// Search for `-a` or `-acc` and return the succeeding argument
    pub fn get_acc(&self) -> Option<String> {
        self.get_parameter(&["-a", "-acc"])
    }

    /// Search for `-grade` and return the succeeding argument
    pub fn get_grade(&self) -> Option<String> {
        self.get_parameter(&["-grade"])
    }

    /// Check if `-g` or `--global` is in the arguments
    pub fn get_global(&self) -> bool {
        self.contains_any(&["-g", "-global"])
    }

    /// Name __must__ be the first argument
    pub fn get_name(&mut self) -> Option<String> {
        self.args.restore();
        self.args.trimmed().single_quoted().ok()
    }

    /// Check if the next argument can be interpreted as ChannelId and return it
    pub fn get_next_channel(&mut self) -> Option<ChannelId> {
        if let Ok(val) = self.args.single::<String>() {
            if let Ok(id) = u64::from_str(&val) {
                Some(ChannelId(id))
            } else {
                let regex = Regex::new(r"<#([0-9]*)>$").unwrap();
                let caps = regex.captures(&val).unwrap();
                caps.get(1)
                    .and_then(|id| u64::from_str(id.as_str()).ok())
                    .map(ChannelId)
            }
        } else {
            None
        }
    }

    /// Check if the next argument can be interpreted as u64 and return it
    pub fn get_next_u64(&mut self) -> Option<u64> {
        self.args.single().ok()
    }

    /// Check if the next argument can be interpreted as RoleId and return it
    pub fn get_next_role(&mut self) -> Option<RoleId> {
        if let Ok(val) = self.args.single::<String>() {
            if let Ok(id) = u64::from_str(&val) {
                Some(RoleId(id))
            } else {
                let regex = Regex::new(r"<@&([0-9]*)>$").unwrap();
                let caps = regex.captures(&val).unwrap();
                caps.get(1)
                    .and_then(|id| u64::from_str(id.as_str()).ok())
                    .map(RoleId)
            }
        } else {
            None
        }
    }

    fn get_parameter(&self, keywords: &[&str]) -> Option<String> {
        if self.args.is_empty() {
            return None;
        }
        let args: Vec<&str> = self.args.raw_quoted().collect();
        for i in 0..args.len() - 1 {
            if keywords.contains(&args[i]) {
                return Some(args[i + 1].to_owned());
            }
        }
        None
    }

    fn contains_any(&self, words: &[&str]) -> bool {
        let args: Vec<&str> = self.args.raw_quoted().collect();
        for word in words {
            if args.contains(word) {
                return true;
            }
        }
        false
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
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
