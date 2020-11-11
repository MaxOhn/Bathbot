use super::Args;
use crate::{
    custom_client::{OsuStatsListParams, OsuStatsOrder, OsuStatsParams},
    util::matcher,
    Context,
};

use rosu::model::GameMode;
use std::str::FromStr;

pub struct OsuStatsListArgs {
    pub params: OsuStatsListParams,
}

impl OsuStatsListArgs {
    pub fn new(args: Args, mode: GameMode) -> Result<Self, &'static str> {
        let mut country = None;
        let mut rank_min = None;
        let mut rank_max = None;
        let mut args = args.take(3);
        while let Some(arg) = args.next() {
            if arg == "-r" || arg == "-rank" {
                if let Some((min, max)) = args.next().and_then(parse_dotted) {
                    rank_min = min;
                    rank_max = Some(max);
                } else {
                    return Err("After the rank keyword you must specify either \
                                an integer for max rank or two decimal numbers of the \
                                form `a..b` for min and max rank");
                }
            } else if country.is_none() {
                country = Some(arg.to_uppercase());
            } else {
                break;
            }
        }
        // Put values into parameter variable
        let mut params = OsuStatsListParams::new(country).mode(mode);
        if let Some(rank_min) = rank_min {
            params = params.rank_min(rank_min);
        }
        if let Some(rank_max) = rank_max {
            params = params.rank_max(rank_max);
        }
        Ok(Self { params })
    }
}

pub struct OsuStatsArgs {
    pub params: OsuStatsParams,
}

impl OsuStatsArgs {
    pub fn new(
        ctx: &Context,
        args: Args,
        mut username: Option<String>,
        mode: GameMode,
    ) -> Result<Self, &'static str> {
        let mut args: Vec<_> = args.take(8).map(|arg| arg.to_owned()).collect();
        // Parse min and max rank
        let mut rank_min = None;
        let mut rank_max = None;
        if let Some(idx) = args.iter().position(|arg| arg == "-r" || arg == "-rank") {
            args.remove(idx);
            if let Some((min, max)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                rank_min = min;
                rank_max = Some(max);
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
            if let Some((min, max)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                acc_min = min;
                acc_max = Some(max);
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
            username = matcher::get_mention_user(&name)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(name));
        }
        if username.is_none() {
            return Err("Either specify an osu name or link your discord \
                        to an osu profile via `link osuname`");
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
        if let Some(selection) = mods {
            params = params.mods(selection);
        }
        Ok(Self { params })
    }
}

fn parse_dotted<T: FromStr>(arg: impl AsRef<str>) -> Option<(Option<T>, T)> {
    let mut split = arg.as_ref().split("..");
    let val = T::from_str(split.next()?).ok()?;
    match split.next() {
        Some(another) => Some((Some(val), T::from_str(another).ok()?)),
        None => Some((None, val)),
    }
}
