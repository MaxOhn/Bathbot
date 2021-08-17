mod args;
mod stream;

pub use args::Args;
pub use stream::Stream;

use crate::{
    // commands::osu::TopSortBy,
    custom_client::{OsuStatsListParams, OsuStatsOrder, OsuStatsParams, SnipeScoreOrder},
    util::{
        matcher,
        osu::{MapIdType, ModSelection},
    },
    Context,
    Name,
};

use itertools::Itertools;
use rosu_v2::model::{
    beatmap::{BeatmapsetSearchSort, Genre, Language, RankStatus},
    GameMode, Grade,
};
use smallstr::SmallString;
use std::{cmp::Ordering, str::FromStr};
use twilight_model::id::{ChannelId, MessageId, RoleId};

pub struct BwsArgs {
    pub name: Option<Name>,
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

pub struct SearchRankStatus(Option<RankStatus>);

impl SearchRankStatus {
    pub fn status(&self) -> Option<RankStatus> {
        self.0
    }
}

pub struct MapSearchArgs {
    pub query: Option<String>,
    pub mode: Option<GameMode>,
    pub status: Option<SearchRankStatus>,
    pub genre: Option<Genre>,
    pub language: Option<Language>,
    pub video: bool,
    pub storyboard: bool,
    pub nsfw: bool,
    pub sort: BeatmapsetSearchSort,
    pub descending: bool,
}

impl MapSearchArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut query = String::with_capacity(args.rest().len());

        let chars = args
            .rest()
            .chars()
            .skip_while(|c| c.is_whitespace())
            .map(|c| c.to_ascii_lowercase());

        query.extend(chars);

        let mode = match query.find("mode=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let mode = match &query[start + "mode=".len()..end] {
                    "0" | "osu" | "std" | "standard" => GameMode::STD,
                    "1" | "tko" | "taiko" => GameMode::TKO,
                    "2" | "ctb" | "fruits" | "catch" => GameMode::CTB,
                    "3" | "mna" | "mania" => GameMode::MNA,
                    _ => {
                        let msg = "Could not parse mode. After `mode=` you must \
                        specify the mode either by its name or by its number i.e. \
                        0=osu, 1=taiko, 2=ctb, 3=mania.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(mode)
            }
            None => None,
        };

        let status = match query.find("status=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let status = match &query[start + "status=".len()..end] {
                    "ranked" => Some(SearchRankStatus(Some(RankStatus::Ranked))),
                    "loved" => Some(SearchRankStatus(Some(RankStatus::Loved))),
                    "qualified" => Some(SearchRankStatus(Some(RankStatus::Qualified))),
                    "pending" | "wip" => Some(SearchRankStatus(Some(RankStatus::Pending))),
                    "graveyard" => Some(SearchRankStatus(Some(RankStatus::Graveyard))),
                    "any" => Some(SearchRankStatus(None)),
                    "leaderboard" => None,
                    _ => {
                        let msg = "Could not parse status. After `status=` you must \
                        specify any of the following options: `ranked`, `loved`, `qualified`, \
                        `pending`, `graveyard`, `any`, or `leaderboard`";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                status
            }
            None => None,
        };

        let genre = match query.find("genre=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let genre = match &query[start + "genre=".len()..end] {
                    "any" => Genre::Any,
                    "unspecified" => Genre::Unspecified,
                    "videogame" | "videogames" => Genre::VideoGame,
                    "anime" => Genre::Anime,
                    "rock" => Genre::Rock,
                    "pop" => Genre::Pop,
                    "other" => Genre::Other,
                    "novelty" => Genre::Novelty,
                    "hiphop" => Genre::HipHop,
                    "electronic" => Genre::Electronic,
                    "metal" => Genre::Metal,
                    "classical" => Genre::Classical,
                    "folk" => Genre::Folk,
                    "jazz" => Genre::Jazz,
                    _ => {
                        let msg = "Could not parse genre. After `genre=` you must \
                        specify any of the following options: `any`, `unspecified`, \
                        `videogame`, `anime`, `rock`, `pop`, `other`, `novelty`, `hiphop`, \
                        `electronic`, `metal`, `classical`, `folk`, or `jazz`.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(genre)
            }
            None => None,
        };

        let language = match query.find("language=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let language = match &query[start + "language=".len()..end] {
                    "any" => Language::Any,
                    "english" => Language::English,
                    "chinese" => Language::Chinese,
                    "french" => Language::French,
                    "german" => Language::German,
                    "italian" => Language::Italian,
                    "japanese" => Language::Japanese,
                    "korean" => Language::Korean,
                    "spanish" => Language::Spanish,
                    "swedish" => Language::Swedish,
                    "russian" => Language::Russian,
                    "polish" => Language::Polish,
                    "instrumental" => Language::Instrumental,
                    "unspecified" => Language::Unspecified,
                    "other" => Language::Other,
                    _ => {
                        let msg = "Could not parse language. After `language=` you must \
                        specify any of the following options: `any`, `english`, `chinese`, \
                        `french`, `german`, `italian`, `japanese`, `korean`, `spanish`, `swdish`, \
                        `russian`, `polish`, `instrumental`, `unspecified`, or `other`.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(language)
            }
            None => None,
        };

        let video = match query.find("video=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let video = match query[start + "video=".len()..end].parse() {
                    Ok(video) => video,
                    Err(_) => {
                        let msg = "Could not parse video boolean. After `video=` \
                        you must specify either `true` or `false`.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                video
            }
            None => false,
        };

        let storyboard = match query.find("storyboard=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let storyboard = match query[start + "storyboard=".len()..end].parse() {
                    Ok(storyboard) => storyboard,
                    Err(_) => {
                        let msg = "Could not parse storyboard boolean. After `storyboard=` \
                        you must specify either `true` or `false`.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                storyboard
            }
            None => false,
        };

        let nsfw = match query.find("nsfw=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let nsfw = match query[start + "nsfw=".len()..end].parse() {
                    Ok(nsfw) => nsfw,
                    Err(_) => {
                        let msg = "Could not parse nsfw boolean. After `nsfw=` \
                        you must specify either `true` or `false`.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                nsfw
            }
            None => true,
        };

        let sort = match query.find("sort=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let sort = match &query[start + "sort=".len()..end] {
                    "artist" => BeatmapsetSearchSort::Artist,
                    "favourites" => BeatmapsetSearchSort::Favourites,
                    "playcount" | "plays" => BeatmapsetSearchSort::Playcount,
                    "rankeddate" | "ranked" => BeatmapsetSearchSort::RankedDate,
                    "rating" => BeatmapsetSearchSort::Rating,
                    "relevance" => BeatmapsetSearchSort::Relevance,
                    "stars" | "difficulty" => BeatmapsetSearchSort::Stars,
                    "title" => BeatmapsetSearchSort::Title,
                    _ => {
                        let msg = "Could not parse sort. After `sort=` you must \
                        specify any of the following options: `artist`, `favourites`, `playcount`, \
                        `rankeddate`, `rating`, `relevance`, `difficulty`, or `title`.";

                        return Err(msg);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                sort
            }
            None => BeatmapsetSearchSort::Relevance,
        };

        let descending = match query.find("-asc") {
            Some(start) => {
                let end = start + "-asc".len();
                let descending = query.len() < end && query.as_bytes()[end] != b' ';

                if !descending {
                    query.replace_range(start..end + (query.len() > end + 1) as usize, "");
                }

                descending
            }
            None => true,
        };

        let trailing_whitespace = query
            .chars()
            .rev()
            .take_while(char::is_ascii_whitespace)
            .count();

        if trailing_whitespace > 0 {
            query.truncate(query.len() - trailing_whitespace);
        }

        let preceeding_whitespace = query.chars().take_while(char::is_ascii_whitespace).count();

        if preceeding_whitespace > 0 {
            query.replace_range(..preceeding_whitespace, "");
        }

        let query = (!query.is_empty()).then(|| query);

        Ok(Self {
            query,
            mode,
            status,
            genre,
            language,
            video,
            storyboard,
            nsfw,
            sort,
            descending,
        })
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
            .and_then(|num| usize::from_str(num).ok())
            .unwrap_or(2);

        Ok(Self { match_id, warmups })
    }
}

pub struct MultNameArgs {
    pub names: Vec<Name>,
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
    pub names: Vec<Name>,
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

pub struct NameArgs {
    pub name: Option<Name>,
}

impl NameArgs {
    pub fn new(ctx: &Context, mut args: Args) -> Self {
        let name = try_link_name(ctx, args.next());

        Self { name }
    }
}

pub struct NameDashPArgs {
    pub name: Option<Name>,
    pub has_dash_p: bool,
}

impl NameDashPArgs {
    pub fn new(ctx: &Context, mut args: Args) -> Self {
        let mut name = None;
        let mut has_dash_p = false;

        match args.next() {
            Some("-p") => has_dash_p = true,
            arg => name = try_link_name(ctx, arg),
        }

        has_dash_p |= args.next().filter(|&arg| arg == "-p").is_some();

        Self { name, has_dash_p }
    }
}

pub struct NameFloatArgs {
    pub name: Option<Name>,
    pub float: f32,
}

impl NameFloatArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args = args.take_all();

        let float = match args.next_back().and_then(|arg| f32::from_str(arg).ok()) {
            Some(float) => float,
            None => return Err("You need to provide a decimal number as last argument"),
        };

        let name = try_link_name(ctx, args.next());

        Ok(Self { name, float })
    }
}

pub struct NameGradePassArgs {
    pub name: Option<Name>,
    pub grade: Option<GradeArg>,
}

impl NameGradePassArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args: Vec<_> = args.take(3).collect();

        let mut grade = None;

        if keywords(&mut args, &["-pass", "-passes"]) {
            grade = Some(GradeArg::Range {
                bot: Grade::D,
                top: Grade::XH,
            });
        } else if let Some(idx) = args.iter().position(|&arg| arg == "-g" || arg == "-grade") {
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

        let name = args.into_iter().next().and_then(|arg| {
            matcher::get_mention_user(arg)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(SmallString::from_str(arg)))
        });

        Ok(Self { name, grade })
    }
}

pub struct NameIntArgs {
    pub name: Option<Name>,
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

pub struct NameMapModArgs {
    pub name: Option<Name>,
    pub map_id: Option<MapIdType>,
    pub mods: Option<ModSelection>,
}

impl NameMapModArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut map_id = None;
        let mut mods = None;

        for arg in args.take(3) {
            if map_id.is_none() {
                if let Some(id) =
                    matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
                {
                    map_id.replace(id);

                    continue;
                }
            }

            if mods.is_none() {
                if let Some(m) = matcher::get_mods(arg) {
                    mods.replace(m);

                    continue;
                }
            }

            name = name.or_else(|| try_link_name(ctx, Some(arg)));

            if map_id.is_some() && name.is_some() && mods.is_some() {
                break;
            }
        }

        Self { name, map_id, mods }
    }
}

pub struct NameModArgs {
    pub name: Option<Name>,
    pub mods: Option<ModSelection>,
    pub converts: bool,
}

impl NameModArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut mods = None;
        let mut converts = false;

        for arg in args {
            if matches!(arg, "-c" | "-convert" | "-converts") {
                converts = true;

                continue;
            }

            let res = matcher::get_mods(arg);

            if res.is_some() {
                mods = res;
            } else {
                name = try_link_name(ctx, Some(arg));
            }
        }

        Self {
            name,
            mods,
            converts,
        }
    }
}

pub enum RankRange {
    Single(u32),
    Range(u32, u32),
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

impl From<SimulateMapArgs> for SimulateArgs {
    fn from(args: SimulateMapArgs) -> Self {
        Self {
            mods: args.mods,
            score: args.score,
            n300: args.n300,
            n100: args.n100,
            n50: args.n50,
            miss: args.miss,
            acc: args.acc,
            combo: args.combo,
        }
    }
}

impl From<SimulateNameArgs> for SimulateArgs {
    fn from(args: SimulateNameArgs) -> Self {
        Self {
            mods: args.mods,
            score: args.score,
            n300: args.n300,
            n100: args.n100,
            n50: args.n50,
            miss: args.miss,
            acc: args.acc,
            combo: args.combo,
        }
    }
}

pub struct SimulateMapArgs {
    pub map_id: Option<MapIdType>,
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

        let map_id = args.pop().as_deref().and_then(matcher::get_osu_map_id);

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
    pub name: Option<Name>,
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
            matcher::get_mention_user(arg)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(SmallString::from_str(arg)))
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
    pub name: Option<Name>,
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
        let descending = !keywords(&mut args, &["--asc", "--ascending", "—asc", "—ascending"]);
        // Parse order
        let order = if keywords(&mut args, &["--a", "--acc", "—a", "—acc"]) {
            SnipeScoreOrder::Accuracy
        } else if keywords(&mut args, &["--md", "--mapdate", "—md", "—mapdate"]) {
            SnipeScoreOrder::MapApprovalDate
        } else if keywords(
            &mut args,
            &["--m", "--miss", "--misses", "—m", "—miss", "—misses"],
        ) {
            SnipeScoreOrder::Misses
        } else if keywords(&mut args, &["--sd", "--scoredate", "—sd", "—scoredate"]) {
            SnipeScoreOrder::ScoreDate
        } else if keywords(&mut args, &["--s", "--stars", "—s", "—stars"]) {
            SnipeScoreOrder::Stars
        } else if keywords(
            &mut args,
            &["--l", "--len", "--length", "—l", "—len", "—length"],
        ) {
            SnipeScoreOrder::Length
        } else {
            SnipeScoreOrder::Pp
        };

        Self {
            name: args.pop().map(SmallString::from_str),
            order,
            mods,
            descending,
        }
    }
}

#[derive(Copy, Clone)]
pub enum GradeArg {
    Single(Grade),
    Range { top: Grade, bot: Grade },
}

pub fn try_link_name(ctx: &Context, msg: Option<&str>) -> Option<Name> {
    msg.and_then(|arg| {
        matcher::get_mention_user(arg)
            .and_then(|id| ctx.get_link(id))
            .or_else(|| Some(SmallString::from_str(arg)))
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
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-a" || arg == "-acc") {
        args.remove(idx);

        match args.get(idx).map(|arg| f32::from_str(arg.as_ref())) {
            Some(Ok(acc)) => {
                args.remove(idx);

                if (0.0..=100.0).contains(&acc) {
                    Ok(Some(acc))
                } else {
                    Err("Accuracy must be between 0.0 and 100.0")
                }
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
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-c" || arg == "-combo") {
        args.remove(idx);

        match args.get(idx).map(|arg| u32::from_str(arg.as_ref())) {
            Some(Ok(combo)) => {
                args.remove(idx);

                Ok(Some(combo))
            }
            Some(Err(_)) => Err("Could not parse given combo, try a non-negative integer"),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn n300(args: &mut Vec<impl AsRef<str>>) -> Result<Option<u32>, &'static str> {
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-300" || arg == "-n300") {
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
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-100" || arg == "-n100") {
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
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-50" || arg == "-n50") {
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
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-s" || arg == "-score") {
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
    let mut arg_refs = args.iter().map(AsRef::as_ref);

    if let Some(idx) = arg_refs.position(|arg| arg == "-x" || arg == "-m") {
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
            fn min(self, other: Self) -> Self {
                match self.partial_cmp(&other).unwrap_or(Ordering::Equal) {
                    Ordering::Less | Ordering::Equal => self,
                    Ordering::Greater => other,
                }
            }

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
