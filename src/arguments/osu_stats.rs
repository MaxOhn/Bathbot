use super::{ArgResult, Args};
use crate::custom_client::{OsuStatsOrder, OsuStatsParams};

use rosu::models::GameMode;
use std::{iter::FromIterator, str::FromStr};

pub struct OsuStatsArgs {
    pub params: OsuStatsParams,
}

impl OsuStatsArgs {
    pub fn new(
        args: Args,
        mut username: Option<String>,
        mode: GameMode,
    ) -> Result<Self, &'static str> {
        let mut args = args.take(8).map(|arg| arg.to_owned()).collect();
        // Parse min and max rank
        let mut rank_min = None;
        let mut rank_max = None;
        if let Some(idx) = args.iter().position(|arg| arg == "-r" || arg == "-rank") {
            args.remove(idx);
            if let Some(arg) = args.get(idx) {
                match parse_dotted(arg) {
                    Some((min, max)) => {
                        args.remove(idx);
                        rank_min = min;
                        rank_max = Some(max);
                    }
                    None => {
                        return Err("After the rank keyword you must specify either \
                            an integer for max rank or two integers of the form \
                            `a..b` for min and max rank")
                    }
                }
            } else {
                return Err("After the rank keyword you must specify either \
                    an integer for max rank or two decimal numbers of the \
                    form `a..b` for min and max rank");
            }
        }
        // Parse min and max acc
        let mut acc_min = None;
        let mut acc_max = None;
        if let Some(idx) = args.iter().position(|arg| arg == "-a" || arg == "-acc") {
            args.remove(idx);
            if let Some(arg) = args.get(idx) {
                match parse_dotted(arg) {
                    Some((min, max)) => {
                        args.remove(idx);
                        acc_min = min;
                        acc_max = Some(max);
                    }
                    None => {
                        return Err("After the acc keyword you must specify either \
                            a decimal number for max acc or two decimal numbers \
                            of the form `a..b` for min and max acc")
                    }
                }
            } else {
                return Err("After the acc keyword you must specify either \
                    a decimal number for max acc or two decimal numbers \
                    of the form `a..b` for min and max acc");
            }
        }
        // Parse mods
        let mods = super::mods(&mut args);
        // Parse descending/ascending
        let descending = !super::keywords(&mut args, &["--asc", "--ascending"]);
        // Parse order
        let sort_by = if super::keywords(&mut args, &["--a", "--acc"]) {
            OsuStatsOrder::Accuracy
        } else if super::keywords(&mut args, &["--c", "--combo"]) {
            OsuStatsOrder::Combo
        } else if super::keywords(&mut args, &["--p", "--pp"]) {
            OsuStatsOrder::Pp
        } else if super::keywords(&mut args, &["--r", "--rank"]) {
            OsuStatsOrder::Rank
        } else if super::keywords(&mut args, &["--s", "--score"]) {
            OsuStatsOrder::Score
        } else if super::keywords(&mut args, &["--m", "--miss", "--misses"]) {
            OsuStatsOrder::Misses
        } else {
            OsuStatsOrder::PlayDate
        };
        // Parse username
        if let Some(name) = args.pop() {
            username = Some(name);
        }
        // TODO: Shift username check to command
        if username.is_none() {
            return Err("Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`");
        }
        // Put values into parameter variable
        let mut params = OsuStatsParams::new(username.unwrap())
            .mode(mode)
            .order(sort_by)
            .descending(descending);
        if let Some(acc_min) = acc_min {
            params = params.acc_min(acc_min);
        }
        if let Some(acc_max) = acc_max {
            params = params.acc_max(acc_max);
        }
        if let Some(rank_min) = rank_min {
            params = params.rank_min(rank_min);
        }
        if let Some(rank_max) = rank_max {
            params = params.rank_max(rank_max);
        }
        if let Some((mods, selection)) = mods {
            params = params.mods(mods, selection);
        }
        Ok(Self { params })
    }
}

fn parse_dotted<T: FromStr>(arg: &str) -> Option<(Option<T>, T)> {
    let mut split = arg.split("..");
    let val = T::from_str(split.next()?).ok()?;
    match split.next() {
        Some(another) => Some((Some(val), T::from_str(another).ok()?)),
        None => Some((None, val)),
    }
}
