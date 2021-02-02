mod args;

pub use args::Args;

use crate::{
    commands::osu::TopSortBy,
    custom_client::{
        ManiaVariant, OsuStatsListParams, OsuStatsOrder, OsuStatsParams, SnipeScoreOrder,
    },
    util::{
        matcher,
        osu::{MapIdType, ModSelection},
    },
    Context,
};

use itertools::Itertools;
use rosu::model::{GameMode, Grade};
use std::{cmp::Ordering, str::FromStr};
use twilight_model::id::{ChannelId, MessageId, RoleId};

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
        let mut args: Vec<_> = args.take(8).collect();

        // Parse min and max rank
        let mut rank_min = None;
        let mut rank_max = None;

        if let Some(idx) = args.iter().position(|&arg| arg == "-r" || arg == "-rank") {
            args.remove(idx);
            if let Some((min, max)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                rank_min = min;
                rank_max = Some(max);
            } else {
                return Err("After the rank keyword you must specify either \
                            an integer for max rank or two integer numbers of the \
                            form `a..b` for min and max rank");
            }
        }

        // Parse min and max acc
        let mut acc_min = None;
        let mut acc_max = None;

        if let Some(idx) = args.iter().position(|&arg| arg == "-a" || arg == "-acc") {
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
        let mods = mods(&mut args);
        // Parse descending/ascending
        let descending = !keywords(&mut args, &["--asc", "--ascending"]);

        // Parse order
        let sort_by = if keywords(&mut args, &["--a", "--acc"]) {
            OsuStatsOrder::Accuracy
        } else if keywords(&mut args, &["--c", "--combo"]) {
            OsuStatsOrder::Combo
        } else if keywords(&mut args, &["--p", "--pp"]) {
            OsuStatsOrder::Pp
        } else if keywords(&mut args, &["--r", "--rank"]) {
            OsuStatsOrder::Rank
        } else if keywords(&mut args, &["--s", "--score"]) {
            OsuStatsOrder::Score
        } else if keywords(&mut args, &["--m", "--miss", "--misses"]) {
            OsuStatsOrder::Misses
        } else {
            OsuStatsOrder::PlayDate
        };

        // Parse username
        if let Some(name) = args.pop() {
            username = matcher::get_mention_user(&name)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(name.to_owned()));
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

pub struct SimulateArgs {
    pub mods: Option<ModSelection>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

impl SimulateArgs {
    #[inline]
    pub fn is_some(&self) -> bool {
        self.acc.is_some()
            || self.mods.is_some()
            || self.combo.is_some()
            || self.miss.is_some()
            || self.score.is_some()
            || self.n300.is_some()
            || self.n100.is_some()
            || self.n50.is_some()
    }
}

impl Into<SimulateArgs> for SimulateMapArgs {
    fn into(self) -> SimulateArgs {
        SimulateArgs {
            mods: self.mods,
            score: self.score,
            n300: self.n300,
            n100: self.n100,
            n50: self.n50,
            miss: self.miss,
            acc: self.acc,
            combo: self.combo,
        }
    }
}

impl Into<SimulateArgs> for SimulateNameArgs {
    fn into(self) -> SimulateArgs {
        SimulateArgs {
            mods: self.mods,
            score: self.score,
            n300: self.n300,
            n100: self.n100,
            n50: self.n50,
            miss: self.miss,
            acc: self.acc,
            combo: self.combo,
        }
    }
}

pub struct SimulateMapArgs {
    pub map_id: Option<u32>,
    pub mods: Option<ModSelection>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

impl SimulateMapArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take(16).collect();
        let mods = mods(&mut args);
        let acc = acc(&mut args)?;
        let combo = combo(&mut args)?;
        let miss = miss(&mut args)?;
        let n300 = n300(&mut args)?;
        let n100 = n100(&mut args)?;
        let n50 = n50(&mut args)?;
        let score = score(&mut args)?;

        let map_id = args
            .pop()
            .as_deref()
            .and_then(matcher::get_osu_map_id)
            .map(|id| id.id());

        Ok(Self {
            map_id,
            mods,
            acc,
            combo,
            score,
            miss,
            n300,
            n100,
            n50,
        })
    }
}

pub struct SimulateNameArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

impl SimulateNameArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args = args.take(16).collect();
        let mods = mods(&mut args);
        let acc = acc(&mut args)?;
        let combo = combo(&mut args)?;
        let miss = miss(&mut args)?;
        let n300 = n300(&mut args)?;
        let n100 = n100(&mut args)?;
        let n50 = n50(&mut args)?;
        let score = score(&mut args)?;

        let name = args.pop().and_then(|arg| {
            matcher::get_mention_user(&arg)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(arg.to_owned()))
        });

        Ok(Self {
            name,
            mods,
            acc,
            combo,
            score,
            miss,
            n300,
            n100,
            n50,
        })
    }
}

pub struct SnipeScoreArgs {
    pub name: Option<String>,
    pub order: SnipeScoreOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl SnipeScoreArgs {
    pub fn new(args: Args) -> Self {
        let mut args: Vec<_> = args.take(4).collect();
        // Parse mods
        let mods = mods(&mut args);
        // Parse descending/ascending
        let descending = !keywords(&mut args, &["--asc", "--ascending"]);
        // Parse order
        let order = if keywords(&mut args, &["--a", "--acc"]) {
            SnipeScoreOrder::Accuracy
        } else if keywords(&mut args, &["--md", "--mapdate"]) {
            SnipeScoreOrder::MapApprovalDate
        } else if keywords(&mut args, &["--m", "--miss", "--misses"]) {
            SnipeScoreOrder::Misses
        } else if keywords(&mut args, &["--sd", "--scoredate"]) {
            SnipeScoreOrder::ScoreDate
        } else if keywords(&mut args, &["--s", "--stars"]) {
            SnipeScoreOrder::Stars
        } else if keywords(&mut args, &["--l", "--len", "--length"]) {
            SnipeScoreOrder::Length
        } else {
            SnipeScoreOrder::Pp
        };

        Self {
            name: args.pop().map(str::to_owned),
            order,
            mods,
            descending,
        }
    }
}

pub struct MatchArgs {
    pub match_id: u32,
    pub warmups: usize,
}

impl MatchArgs {
    pub fn new(mut args: Args) -> Result<Self, &'static str> {
        let match_id = match args.next().and_then(|arg| matcher::get_osu_match_id(arg)) {
            Some(id) => id,
            None => {
                return Err("The first argument must be either a match \
                        id or the multiplayer link to a match")
            }
        };

        let warmups = args
            .next()
            .and_then(|num| usize::from_str(&num).ok())
            .unwrap_or(2);

        Ok(Self { match_id, warmups })
    }
}

pub struct MapModArgs {
    pub map_id: Option<MapIdType>,
    pub mods: Option<ModSelection>,
}

impl MapModArgs {
    pub fn new(args: Args) -> Self {
        let mut map_id = None;
        let mut mods = None;

        for arg in args {
            let maybe_map_id =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg));

            let maybe_mods = match maybe_map_id {
                Some(_) => None,
                None => matcher::get_mods(arg),
            };

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
    pub map_id: Option<MapIdType>,
}

impl NameMapArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut map_id = None;

        for arg in args {
            if map_id.is_none() {
                if let Some(id) =
                    matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
                {
                    map_id = Some(id);
                    continue;
                }
            }
            name = name.or_else(|| try_link_name(ctx, Some(arg)));

            if map_id.is_some() && name.is_some() {
                break;
            }
        }

        Self { name, map_id }
    }
}

pub struct RoleAssignArgs {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    pub role_id: RoleId,
}

impl RoleAssignArgs {
    pub fn new(mut args: Args) -> Result<Self, &'static str> {
        let channel_id = args
            .next()
            .and_then(|arg| matcher::get_mention_channel(arg))
            .map(ChannelId);

        if channel_id.is_none() {
            return Err("Could not parse channel. Make sure your \
                        first argument is either a channel mention \
                        or a channel id.");
        }

        let message_id = args
            .next()
            .and_then(|arg| u64::from_str(arg).ok())
            .map(MessageId);

        if message_id.is_none() {
            return Err("Could not parse message. Make sure your \
                        second argument is a message id.");
        }

        let role_id = args
            .next()
            .and_then(|arg| matcher::get_mention_role(arg))
            .map(RoleId);

        if role_id.is_none() {
            return Err("Could not parse role. Make sure your \
                        third argument is either a role mention \
                        or a role id.");
        }

        Ok(Self {
            channel_id: channel_id.unwrap(),
            message_id: message_id.unwrap(),
            role_id: role_id.unwrap(),
        })
    }
}

pub struct NameArgs {
    pub name: Option<String>,
}

impl NameArgs {
    #[inline]
    pub fn new(ctx: &Context, mut args: Args) -> Self {
        let name = try_link_name(ctx, args.next());

        Self { name }
    }
}

pub struct MultNameArgs {
    pub names: Vec<String>,
}

impl MultNameArgs {
    pub fn new(ctx: &Context, args: Args, n: usize) -> Self {
        let names = args
            .take(n)
            .unique()
            .map(|arg| try_link_name(ctx, Some(arg)).unwrap())
            .collect();

        Self { names }
    }
}

pub struct MultNameLimitArgs {
    pub names: Vec<String>,
    pub limit: Option<usize>,
}

impl MultNameLimitArgs {
    pub fn new(ctx: &Context, args: Args, n: usize) -> Result<Self, &'static str> {
        let mut args: Vec<_> = args.take_all().unique().take(n + 2).collect();

        let limit = match args
            .iter()
            .position(|&arg| arg == "-limit" || arg == "-l" || arg == "-top" || arg == "-t")
        {
            Some(idx) => {
                args.remove(idx);

                match args.get(idx).map(|&arg| usize::from_str(arg)) {
                    Some(Ok(limit)) => {
                        args.remove(idx);
                        Some(limit)
                    }
                    Some(Err(_)) => {
                        return Err("Could not parse given limit, try a non-negative integer")
                    }
                    None => None,
                }
            }
            None => None,
        };

        let names = args
            .into_iter()
            .map(|arg| try_link_name(ctx, Some(arg)).unwrap())
            .collect();

        Ok(Self { names, limit })
    }
}

pub struct NameFloatArgs {
    pub name: Option<String>,
    pub float: f32,
}

impl NameFloatArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args = args.take_all();

        let float = match args.next_back().and_then(|arg| f32::from_str(&arg).ok()) {
            Some(float) => float,
            None => return Err("You need to provide a decimal number as last argument"),
        };

        let name = try_link_name(ctx, args.next());

        Ok(Self { name, float })
    }
}

pub struct NameIntArgs {
    pub name: Option<String>,
    pub number: Option<u32>,
}

impl NameIntArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut number = None;

        for arg in args {
            let res = u32::from_str(arg).ok();
            if res.is_some() {
                number = res;
            } else {
                name = try_link_name(ctx, Some(arg));
            }
        }

        Self { name, number }
    }
}

pub struct NameModArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
}

impl NameModArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut mods = None;

        for arg in args {
            let res = matcher::get_mods(arg);
            if res.is_some() {
                mods = res;
            } else {
                name = try_link_name(ctx, Some(arg));
            }
        }

        Self { name, mods }
    }
}

#[derive(Copy, Clone)]
pub enum GradeArg {
    Single(Grade),
    Range { top: Grade, bot: Grade },
}

pub struct TopArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
    pub acc_min: Option<f32>,
    pub acc_max: Option<f32>,
    pub combo_min: Option<u32>,
    pub combo_max: Option<u32>,
    pub grade: Option<GradeArg>,
    pub sort_by: TopSortBy,
    pub has_dash_r: bool,
    pub has_dash_p: bool,
}

impl TopArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args: Vec<_> = args.take(10).collect();

        let mut acc_min = None;
        let mut acc_max = None;

        if let Some(idx) = args.iter().position(|&arg| arg == "-a") {
            args.remove(idx);
            if let Some((min, minmax)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                if let Some(min) = min {
                    acc_min.replace(min);
                    acc_max.replace(minmax);
                } else {
                    acc_min.replace(minmax);
                }
            } else {
                return Err("After the acc keyword you must specify either \
                    a decimal number for min acc or two decimal numbers \
                    of the form `a..b` for min and max acc");
            }
        }

        let mut combo_min = None;
        let mut combo_max = None;

        if let Some(idx) = args.iter().position(|&arg| arg == "-c") {
            args.remove(idx);
            if let Some((min, minmax)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                if let Some(min) = min {
                    combo_min.replace(min);
                    combo_max.replace(minmax);
                } else {
                    combo_min.replace(minmax);
                }
            } else {
                return Err("After the combo keyword you must specify either \
                            an integer for min combo or two integer numbers of the \
                            form `a..b` for min and max combo");
            }
        }

        let mut grade = None;

        if let Some(idx) = args.iter().position(|&arg| arg == "-g" || arg == "-grade") {
            args.remove(idx);
            if let Some((min, mut max)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);

                match min {
                    Some(mut min) => {
                        if min == Grade::SH {
                            min = Grade::S;
                        } else if min == Grade::XH {
                            min = Grade::X;
                        }

                        if max == Grade::S {
                            max = Grade::SH;
                        } else if max == Grade::X {
                            max = Grade::XH;
                        }

                        grade.replace(GradeArg::Range { bot: min, top: max })
                    }
                    None => grade.replace(GradeArg::Single(max)),
                };
            } else {
                return Err("Could not parse given grade, try SS, S, A, B, C, or D");
            }
        }

        let mods = mods(&mut args);

        let sort_by = if keywords(&mut args, &["--a", "--acc"]) {
            TopSortBy::Acc
        } else if keywords(&mut args, &["--c", "--combo"]) {
            TopSortBy::Combo
        } else {
            TopSortBy::None
        };

        let has_dash_r = keywords(&mut args, &["-r"]);
        let has_dash_p = keywords(&mut args, &["-p"]);

        let name = args.pop().and_then(|arg| {
            matcher::get_mention_user(&arg)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(arg.to_owned()))
        });

        Ok(Self {
            name,
            mods,
            acc_min,
            acc_max,
            combo_min,
            combo_max,
            grade,
            sort_by,
            has_dash_r,
            has_dash_p,
        })
    }
}

pub struct BwsArgs {
    pub name: Option<String>,
    pub rank_range: Option<RankRange>,
}

impl BwsArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut rank_range = None;

        for arg in args {
            match parse_dotted(arg) {
                Some((Some(min), max)) => rank_range = Some(RankRange::Range(min, max)),
                Some((None, rank)) => rank_range = Some(RankRange::Single(rank)),
                None => {
                    if name.is_none() {
                        name = try_link_name(ctx, Some(arg));
                    }
                }
            }
        }

        Self { name, rank_range }
    }
}

pub enum RankRange {
    Single(u32),
    Range(u32, u32),
}

pub struct RankArgs {
    pub name: Option<String>,
    pub country: Option<String>,
    pub rank: usize,
    pub variant: Option<ManiaVariant>,
}

impl RankArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args = args.take_n(3);

        let (country, rank) = if let Some(arg) = args.next_back() {
            if arg.starts_with('+') {
                return Err("Could not parse rank. Be sure to specify it as *last* argument.");
            } else if let Ok(num) = arg.parse() {
                (None, num)
            } else if arg.len() < 3 {
                return Err(
                    "Could not parse rank. Provide it either as positive number \
                    or as country acronym followed by a positive number e.g. `be10`.",
                );
            } else {
                let (country, num) = arg.split_at(2);

                match (num.parse(), country.chars().all(|c| c.is_ascii_alphabetic())) {
                    (Ok(num), true) => (Some(country.to_uppercase()), num),
                    (Err(_), _) => {
                        return Err(
                            "Could not parse rank. Provide it either as positive number \
                            or as country acronym followed by a positive number e.g. `be10`."
                        )
                    }
                    (_, false) => {
                        return Err(
                            "Could not parse country. Be sure to specify it with two letters, e.g. `be10`.",
                        )
                    }
                }
            }
        } else {
            return Err(
                "No rank argument found. Provide it either as positive number or \
                 as country acronym followed by a positive number e.g. `be10`.",
            );
        };

        let (name, variant) = match (args.next(), args.next()) {
            (None, None) => (None, None),
            (Some(arg), None) => match arg.parse() {
                Ok(variant) => (None, Some(variant)),
                Err(_) => (try_link_name(ctx, Some(arg)), None),
            },
            (Some(arg1), Some(arg2)) => {
                if let Ok(variant) = arg2.parse() {
                    (try_link_name(ctx, Some(arg1)), Some(variant))
                } else if let Ok(variant) = arg1.parse() {
                    (try_link_name(ctx, Some(arg2)), Some(variant))
                } else {
                    return Err(
                        "If three arguments are provided, I expect one of them to be the \
                        mania variant `+4k` or `+7k` but I could not find any of them.",
                    );
                }
            }
            (None, Some(_)) => unreachable!(),
        };

        Ok(Self {
            name,
            country,
            rank,
            variant,
        })
    }
}

pub fn try_link_name(ctx: &Context, msg: Option<&str>) -> Option<String> {
    msg.and_then(|arg| {
        matcher::get_mention_user(arg)
            .and_then(|id| ctx.get_link(id))
            .or_else(|| Some(arg.to_owned()))
    })
}

fn mods(args: &mut Vec<impl AsRef<str>>) -> Option<ModSelection> {
    for (i, arg) in args.iter().enumerate() {
        let mods = matcher::get_mods(arg.as_ref());

        if mods.is_some() {
            args.remove(i);
            return mods;
        }
    }

    None
}

fn acc(args: &mut Vec<impl AsRef<str>>) -> Result<Option<f32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-a" || arg == "-acc"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| f32::from_str(arg.as_ref())) {
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

fn combo(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-c" || arg == "-combo"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
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

fn n300(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-300" || arg == "-n300"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
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

fn n100(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-100" || arg == "-n100"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
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

fn n50(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-50" || arg == "-n50"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
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

fn score(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-s" || arg == "-score"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
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

fn miss(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    if let Some(idx) = args.iter().position(|arg| {
        let arg = arg.as_ref();

        arg == "-x" || arg == "-m"
    }) {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
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

fn keywords(args: &mut Vec<impl AsRef<str>>, keys: &[&str]) -> bool {
    if let Some(idx) = args.iter().position(|arg| keys.contains(&arg.as_ref())) {
        args.remove(idx);
        return true;
    }

    false
}

fn parse_dotted<T: DottedValue>(arg: impl AsRef<str>) -> Option<(Option<T>, T)> {
    let mut split = arg.as_ref().split("..");
    let val = T::from_str(split.next()?).ok()?;

    match split.next() {
        Some(another) => {
            let other = T::from_str(another).ok()?;

            Some((Some(val.min(other)), val.max(other)))
        }
        None => Some((None, val)),
    }
}

trait DottedValue: PartialOrd + FromStr + Copy {
    fn min(self, other: Self) -> Self;
    fn max(self, other: Self) -> Self;
}

macro_rules! impl_dotted_value {
    ($type:ty) => {
        impl DottedValue for $type {
            #[inline]
            fn min(self, other: Self) -> Self {
                match self.partial_cmp(&other).unwrap_or(Ordering::Equal) {
                    Ordering::Less | Ordering::Equal => self,
                    Ordering::Greater => other,
                }
            }

            #[inline]
            fn max(self, other: Self) -> Self {
                match self.partial_cmp(&other).unwrap_or(Ordering::Equal) {
                    Ordering::Less | Ordering::Equal => other,
                    Ordering::Greater => self,
                }
            }
        }
    };
}

impl_dotted_value!(Grade);
impl_dotted_value!(u32);
impl_dotted_value!(f32);
impl_dotted_value!(usize);
