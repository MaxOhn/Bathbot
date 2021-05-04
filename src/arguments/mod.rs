mod args;
mod stream;

pub use args::Args;
pub use stream::Stream;

use crate::{
    commands::osu::TopSortBy,
    custom_client::{OsuStatsListParams, OsuStatsOrder, OsuStatsParams, SnipeScoreOrder},
    util::{
        matcher,
        osu::{MapIdType, ModSelection},
    },
    Context, Name,
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
            .and_then(|num| usize::from_str(&num).ok())
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
    #[inline]
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
    #[inline]
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

        let float = match args.next_back().and_then(|arg| f32::from_str(&arg).ok()) {
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
            matcher::get_mention_user(&arg)
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

pub struct NameMapArgs {
    pub name: Option<Name>,
    pub map_id: Option<MapIdType>,
}

impl NameMapArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut map_id = None;

        for arg in args.take(2) {
            if map_id.is_none() {
                if let Some(id) =
                    matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
                {
                    map_id.replace(id);

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

pub struct OsuStatsArgs {
    pub params: OsuStatsParams,
}

impl OsuStatsArgs {
    pub fn new(
        ctx: &Context,
        args: Args,
        mut username: Option<Name>,
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
                .or_else(|| Some(SmallString::from_str(name)));
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

pub enum RankRange {
    Single(u32),
    Range(u32, u32),
}

pub struct RankArgs {
    pub name: Option<Name>,
    pub country: Option<String>,
    pub rank: usize,
}

impl RankArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut name = None;
        let mut country_rank = None;

        for arg in args.take(2) {
            if let Ok(num) = arg.parse() {
                country_rank.replace((None, num));

                continue;
            }

            if arg.len() >= 3 {
                let (country, num) = arg.split_at(2);
                let valid_country = country.chars().all(|c| c.is_ascii_alphabetic());

                if let (true, Ok(num)) = (valid_country, num.parse()) {
                    country_rank.replace((Some(country.to_uppercase()), num));

                    continue;
                }
            }

            name = try_link_name(ctx, Some(arg));
        }

        let (country, rank) = country_rank.ok_or(
            "Could not parse rank. Provide it either as positive number \
                or as country acronym followed by a positive number e.g. `be10` \
                as one of the first two arguments.",
        )?;

        Ok(Self {
            name,
            country,
            rank,
        })
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
            matcher::get_mention_user(&arg)
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

pub struct TopArgs {
    pub name: Option<Name>,
    pub mods: Option<ModSelection>,
    pub acc_min: Option<f32>,
    pub acc_max: Option<f32>,
    pub combo_min: Option<u32>,
    pub combo_max: Option<u32>,
    pub grade: Option<GradeArg>,
    pub sort_by: TopSortBy,
    pub has_dash_r: bool,
    pub has_dash_p_or_i: bool,
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
        let has_dash_p_or_i = keywords(&mut args, &["-p", "-i"]);

        let name = args.into_iter().next().and_then(|arg| {
            matcher::get_mention_user(&arg)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(SmallString::from_str(arg)))
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
            has_dash_p_or_i,
        })
    }
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
