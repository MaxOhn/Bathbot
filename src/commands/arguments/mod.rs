#![allow(dead_code)] // TODO: remove line once its used

use serenity::framework::standard::Args;
use std::str::FromStr;

pub struct ArgParser {
    args: Args,
}

impl ArgParser {
    pub fn new(args: Args) -> Self {
        Self { args }
    }

    /// Search for `+mods` (included), `+mods!` (exact), or `-mods!` (excluded)
    pub fn get_mods(&mut self) -> Option<(String, ModSelection)> {
        for arg in self.args.trimmed().iter::<String>() {
            if let Ok(arg) = arg {
                if arg.starts_with('+') {
                    if arg.ends_with('!') {
                        return Some((arg[1..arg.len() - 1].to_owned(), ModSelection::Exact));
                    } else {
                        return Some((
                            arg.trim_start_matches('+').to_owned(),
                            ModSelection::Includes,
                        ));
                    }
                } else if arg.starts_with('-') && arg.ends_with('!') {
                    return Some((arg[1..arg.len() - 1].to_owned(), ModSelection::Excludes));
                }
            }
        }
        None
    }

    /// Search for `-c` or `-combo` and return the succeeding argument
    pub fn get_combo(&self) -> Option<String> {
        self.get_option(&["-c", "-combo"])
    }

    /// Search for `-a` or `-acc` and return the succeeding argument
    pub fn get_acc(&self) -> Option<String> {
        self.get_option(&["-a", "-acc"])
    }

    /// Search for `-grade` and return the succeeding argument
    pub fn get_grade(&self) -> Option<String> {
        self.get_option(&["-grade"])
    }

    /// Name __must__ be the first argument
    pub fn get_name(&mut self) -> Option<String> {
        self.args.restore();
        self.args.trimmed().single_quoted().ok()
    }

    fn get_option(&self, keywords: &[&str]) -> Option<String> {
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
}

pub enum ModSelection {
    None,
    Includes,
    Excludes,
    Exact,
}

pub fn get_beatmap_id(val: String) -> Option<u32> {
    // TODO: Regex here
    Some(u32::from_str(&val).unwrap())
}
