use std::{collections::BTreeMap, sync::Arc};

use bathbot_macros::{command, SlashCommand};
use bathbot_util::constants::OSU_API_ISSUE;
use eyre::{Report, Result};
use rosu_v2::prelude::{
    Beatmapset, BeatmapsetSearchResult, BeatmapsetSearchSort, Genre, Language, Osu, OsuResult,
    RankStatus,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    pagination::MapSearchPagination,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "search")]
/// Search for mapsets
pub struct Search {
    /// Specify a search query
    pub query: Option<String>,
    /// Specify a gamemode
    pub mode: Option<GameModeOption>,
    /// Specify a ranking status
    pub status: Option<SearchStatus>,
    /// Specify the order of mapsets
    pub sort: Option<SearchOrder>,
    /// Specify a genre
    pub genre: Option<SearchGenre>,
    /// Specify a language
    pub language: Option<SearchLanguage>,
    /// Specify if the mapset should have a video
    pub video: Option<bool>,
    /// Specify if the mapset should have a storyboard
    pub storyboard: Option<bool>,
    /// Specify whether the mapset can be NSFW
    pub nsfw: Option<bool>,
    /// Specify whether the resulting list should be reversed
    pub reverse: Option<bool>,
}

#[derive(CommandOption, CreateOption, Debug)]
pub enum SearchStatus {
    #[option(name = "Any", value = "any")]
    Any,
    #[option(name = "Leaderboard", value = "leaderboard")]
    Leaderboard,
    #[option(name = "Ranked", value = "ranked")]
    Ranked,
    #[option(name = "Loved", value = "loved")]
    Loved,
    #[option(name = "Qualified", value = "qualified")]
    Qualified,
    #[option(name = "Pending", value = "pending")]
    Pending,
    #[option(name = "Graveyard", value = "graveyard")]
    Graveyard,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum SearchGenre {
    #[option(name = "Any", value = "any")]
    Any,
    #[option(name = "Anime", value = "anime")]
    Anime,
    #[option(name = "Classical", value = "classical")]
    Classical,
    #[option(name = "Electronic", value = "electronic")]
    Electronic,
    #[option(name = "Folk", value = "folk")]
    Folk,
    #[option(name = "Hip-Hop", value = "hiphop")]
    HipHop,
    #[option(name = "Jazz", value = "jazz")]
    Jazz,
    #[option(name = "Metal", value = "metal")]
    Metal,
    #[option(name = "Novelty", value = "novelty")]
    Novelty,
    #[option(name = "Other", value = "other")]
    Other,
    #[option(name = "Pop", value = "pop")]
    Pop,
    #[option(name = "Rock", value = "rock")]
    Rock,
    #[option(name = "Unspecified", value = "unspecified")]
    Unspecified,
    #[option(name = "VideoGame", value = "videogame")]
    VideoGame,
}

impl From<SearchGenre> for Genre {
    fn from(genre: SearchGenre) -> Self {
        match genre {
            SearchGenre::Any => Self::Any,
            SearchGenre::Anime => Self::Anime,
            SearchGenre::Classical => Self::Classical,
            SearchGenre::Electronic => Self::Electronic,
            SearchGenre::Folk => Self::Folk,
            SearchGenre::HipHop => Self::HipHop,
            SearchGenre::Jazz => Self::Jazz,
            SearchGenre::Metal => Self::Metal,
            SearchGenre::Novelty => Self::Novelty,
            SearchGenre::Other => Self::Other,
            SearchGenre::Pop => Self::Pop,
            SearchGenre::Rock => Self::Rock,
            SearchGenre::Unspecified => Self::Unspecified,
            SearchGenre::VideoGame => Self::VideoGame,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum SearchLanguage {
    #[option(name = "Any", value = "any")]
    Any,
    #[option(name = "Chinese", value = "chinese")]
    Chinese,
    #[option(name = "English", value = "english")]
    English,
    #[option(name = "French", value = "french")]
    French,
    #[option(name = "German", value = "german")]
    German,
    #[option(name = "Instrumental", value = "instrumental")]
    Instrumental,
    #[option(name = "Italian", value = "italian")]
    Italian,
    #[option(name = "Japanese", value = "japanese")]
    Japanese,
    #[option(name = "Korean", value = "korean")]
    Korean,
    #[option(name = "Other", value = "other")]
    Other,
    #[option(name = "Polish", value = "polish")]
    Polish,
    #[option(name = "Russian", value = "russian")]
    Russian,
    #[option(name = "Spanish", value = "spanish")]
    Spanish,
    #[option(name = "Swedish", value = "swedish")]
    Swedish,
    #[option(name = "Unspecified", value = "unspecified")]
    Unspecified,
}

impl From<SearchLanguage> for Language {
    fn from(language: SearchLanguage) -> Self {
        match language {
            SearchLanguage::Any => Self::Any,
            SearchLanguage::Chinese => Self::Chinese,
            SearchLanguage::English => Self::English,
            SearchLanguage::French => Self::French,
            SearchLanguage::German => Self::German,
            SearchLanguage::Instrumental => Self::Instrumental,
            SearchLanguage::Italian => Self::Italian,
            SearchLanguage::Japanese => Self::Japanese,
            SearchLanguage::Korean => Self::Korean,
            SearchLanguage::Other => Self::Other,
            SearchLanguage::Polish => Self::Polish,
            SearchLanguage::Russian => Self::Russian,
            SearchLanguage::Spanish => Self::Spanish,
            SearchLanguage::Swedish => Self::Swedish,
            SearchLanguage::Unspecified => Self::Unspecified,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum SearchOrder {
    #[option(name = "Artist", value = "artist")]
    Artist,
    #[option(name = "Favourites", value = "favourites")]
    Favourites,
    #[option(name = "Playcount", value = "playcount")]
    Playcount,
    #[option(name = "RankedDate", value = "ranked_date")]
    RankedDate,
    #[option(name = "Rating", value = "rating")]
    Rating,
    #[option(name = "Relevance", value = "relevance")]
    Relevance,
    #[option(name = "Stars", value = "stars")]
    Stars,
    #[option(name = "Title", value = "title")]
    Title,
}

impl Default for SearchOrder {
    fn default() -> Self {
        Self::Relevance
    }
}

impl From<SearchOrder> for BeatmapsetSearchSort {
    fn from(order: SearchOrder) -> Self {
        match order {
            SearchOrder::Artist => Self::Artist,
            SearchOrder::Favourites => Self::Favourites,
            SearchOrder::Playcount => Self::Playcount,
            SearchOrder::RankedDate => Self::RankedDate,
            SearchOrder::Rating => Self::Rating,
            SearchOrder::Relevance => Self::Relevance,
            SearchOrder::Stars => Self::Stars,
            SearchOrder::Title => Self::Title,
        }
    }
}

impl Search {
    pub fn args(args: Args<'_>) -> Result<Self, &'static str> {
        let args = args.rest();
        let mut query = String::with_capacity(args.len());

        let chars = args
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
                    "0" | "o" | "osu" | "std" | "standard" => GameModeOption::Osu,
                    "1" | "t" | "tko" | "taiko" => GameModeOption::Taiko,
                    "2" | "c" | "ctb" | "fruits" | "catch" => GameModeOption::Catch,
                    "3" | "m" | "mna" | "mania" => GameModeOption::Mania,
                    _ => {
                        let content = "Failed to parse `mode`. After `mode=` you must \
                        specify the mode either by its name or by its number i.e. \
                        0=osu, 1=taiko, 2=ctb, 3=mania.";

                        return Err(content);
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
                    "ranked" => SearchStatus::Ranked,
                    "loved" => SearchStatus::Loved,
                    "qualified" => SearchStatus::Qualified,
                    "pending" | "wip" => SearchStatus::Pending,
                    "graveyard" => SearchStatus::Graveyard,
                    "any" => SearchStatus::Any,
                    "leaderboard" => SearchStatus::Leaderboard,
                    _ => {
                        let content = "Failed to parse `status`. After `status=` you must \
                        specify any of the following options: `ranked`, `loved`, `qualified`, \
                        `pending`, `graveyard`, `any`, or `leaderboard`";

                        return Err(content);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(status)
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
                    "any" => SearchGenre::Any,
                    "unspecified" => SearchGenre::Unspecified,
                    "videogame" | "videogames" => SearchGenre::VideoGame,
                    "anime" => SearchGenre::Anime,
                    "rock" => SearchGenre::Rock,
                    "pop" => SearchGenre::Pop,
                    "other" => SearchGenre::Other,
                    "novelty" => SearchGenre::Novelty,
                    "hiphop" => SearchGenre::HipHop,
                    "electronic" => SearchGenre::Electronic,
                    "metal" => SearchGenre::Metal,
                    "classical" => SearchGenre::Classical,
                    "folk" => SearchGenre::Folk,
                    "jazz" => SearchGenre::Jazz,
                    _ => {
                        let msg = "Failed to parse `genre`. After `genre=` you must \
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
                    "any" => SearchLanguage::Any,
                    "english" => SearchLanguage::English,
                    "chinese" => SearchLanguage::Chinese,
                    "french" => SearchLanguage::French,
                    "german" => SearchLanguage::German,
                    "italian" => SearchLanguage::Italian,
                    "japanese" => SearchLanguage::Japanese,
                    "korean" => SearchLanguage::Korean,
                    "spanish" => SearchLanguage::Spanish,
                    "swedish" => SearchLanguage::Swedish,
                    "russian" => SearchLanguage::Russian,
                    "polish" => SearchLanguage::Polish,
                    "instrumental" => SearchLanguage::Instrumental,
                    "unspecified" => SearchLanguage::Unspecified,
                    "other" => SearchLanguage::Other,
                    _ => {
                        let content = "Failed to parse `language`. After `language=` you must \
                        specify any of the following options: `any`, `english`, `chinese`, \
                        `french`, `german`, `italian`, `japanese`, `korean`, `spanish`, `swdish`, \
                        `russian`, `polish`, `instrumental`, `unspecified`, or `other`.";

                        return Err(content);
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
                        let content = "Failed to parse `video` boolean. After `video=` \
                        you must specify either `true` or `false`.";

                        return Err(content);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(video)
            }
            None => None,
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
                        let content = "Failed to parse `storyboard` boolean. After `storyboard=` \
                        you must specify either `true` or `false`.";

                        return Err(content);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(storyboard)
            }
            None => None,
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
                        let content = "Failed to parse `nsfw` boolean. After `nsfw=` \
                        you must specify either `true` or `false`.";

                        return Err(content);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(nsfw)
            }
            None => None,
        };

        let sort = match query.find("sort=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let sort = match &query[start + "sort=".len()..end] {
                    "artist" => SearchOrder::Artist,
                    "favourites" => SearchOrder::Favourites,
                    "playcount" | "plays" => SearchOrder::Playcount,
                    "rankeddate" | "ranked" => SearchOrder::RankedDate,
                    "rating" => SearchOrder::Rating,
                    "relevance" => SearchOrder::Relevance,
                    "stars" | "difficulty" => SearchOrder::Stars,
                    "title" => SearchOrder::Title,
                    _ => {
                        let content = "Failed to parse `sort`. After `sort=` you must \
                        specify any of the following options: `artist`, `favourites`, `playcount`, \
                        `rankeddate`, `rating`, `relevance`, `difficulty`, or `title`.";

                        return Err(content);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(sort)
            }
            None => None,
        };

        let reverse = match query.find("reverse=") {
            Some(start) => {
                let mut end = start + 1;

                while end < query.len() && query.as_bytes()[end] != b' ' {
                    end += 1;
                }

                let reverse = match &query[start + "reverse=".len()..end] {
                    "true" | "t" | "1" => true,
                    "false" | "f" | "0" => false,
                    _ => {
                        let content = "Failed to parse `reverse`. After `reverse=` \
                        you must specify either `true` or `false`.";

                        return Err(content);
                    }
                };

                query.replace_range(start..end + (query.len() > end + 1) as usize, "");

                Some(reverse)
            }
            None => None,
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

        let query = (!query.is_empty()).then_some(query);

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
            reverse,
        })
    }

    async fn request(&self, osu: &Osu) -> OsuResult<BeatmapsetSearchResult> {
        let sort = self
            .sort
            .map(BeatmapsetSearchSort::from)
            .unwrap_or_default();

        let descending = self.reverse.map_or(true, |r| !r);

        let mut search_fut = osu
            .beatmapset_search()
            .video(self.video.unwrap_or(false))
            .storyboard(self.storyboard.unwrap_or(false))
            .nsfw(self.nsfw.unwrap_or(true))
            .sort(sort, descending);

        if let Some(ref query) = self.query {
            search_fut = search_fut.query(query);
        }

        if let Some(mode) = self.mode {
            search_fut = search_fut.mode(mode.into());
        }

        search_fut = match self.status {
            Some(SearchStatus::Any) => search_fut.any_status(),
            Some(SearchStatus::Leaderboard) | None => search_fut,
            Some(SearchStatus::Ranked) => search_fut.status(RankStatus::Ranked),
            Some(SearchStatus::Loved) => search_fut.status(RankStatus::Loved),
            Some(SearchStatus::Qualified) => search_fut.status(RankStatus::Qualified),
            Some(SearchStatus::Pending) => search_fut.status(RankStatus::Pending),
            Some(SearchStatus::Graveyard) => search_fut.status(RankStatus::Graveyard),
        };

        if let Some(genre) = self.genre {
            search_fut = search_fut.genre(genre.into());
        }

        if let Some(language) = self.language {
            search_fut = search_fut.language(language.into());
        }

        search_fut.await
    }
}

async fn slash_search(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Search::from_interaction(command.input_data())?;

    search(ctx, (&mut command).into(), args).await
}

#[command]
#[desc("Search for mapsets")]
#[help(
    "Search for mapsets. \n\
    The query works essentially the same as in game, meaning you can add \
    any keywords, as well as specific assignments like `creator=abc`, `length<123`, `ar>=9`, ...\n\n\
    Additionally, there are various special arguments you can provide with `argument=abc`:\n\
    - __`mode`__: `osu`, `taiko`, `ctb`, or `mania`, defaults to none\n\
    - __`status`__: `ranked`, `loved`, `qualified`, `pending`, `graveyard`, `any`, or \
    `leaderboard`, defaults to `leaderboard`\n\
    - __`genre`__: `any`, `unspecified`, `videogame`, `anime`, `rock`, `pop`, `other`, `novelty`, \
    `hiphop`, `electronic`, `metal`, `classical`, `folk`, or `jazz`, defaults to `any`\n\
    - __`language`__: `any`, `english`, `chinese`, `french`, `german`, `italian`, `japanese`, \
    `korean`, `spanish`, `swedish`, `russian`, `polish`, `instrumental`, `unspecified`, \
    or `other`, defaults to `any`\n\
    - __`video`__: `true` or `false`, defaults to `false`\n\
    - __`storyboard`__: `true` or `false`, defaults to `false`\n\
    - __`nsfw`__: `true` or `false`, defaults to `true` (allows nsfw, not requires nsfw)\n\
    - __`sort`__: `favourites`, `playcount`, `rankeddate`, `rating`, `relevance`, `stars`, \
    `artist`, or `title`, defaults to `relevance`\n\n\
    Depending on `sort`, the mapsets are ordered in descending order by default. \
    To reverse, specify `reverse=true`."
)]
#[aliases("searchmap", "mapsearch")]
#[usage("[search query]")]
#[examples(
    "some words yay mode=osu status=graveyard sort=favourites reverse=true",
    "artist=camellia length<240 stars>8 genre=electronic"
)]
#[group(AllModes)]
async fn prefix_search(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Search::args(args) {
        Ok(args) => search(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn search(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Search) -> Result<()> {
    let mut search_result = match args.request(ctx.osu()).await {
        Ok(response) => response,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE);
            let report = Report::new(err).wrap_err("failed to get search results");

            return Err(report);
        }
    };

    let maps: BTreeMap<usize, Beatmapset> = search_result.mapsets.drain(..).enumerate().collect();

    MapSearchPagination::builder(maps, search_result, args)
        .map_search_components()
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}
