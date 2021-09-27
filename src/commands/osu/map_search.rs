use crate::{
    commands::SlashCommandBuilder,
    embeds::{EmbedData, MapSearchEmbed},
    pagination::{MapSearchPagination, Pagination},
    util::{constants::OSU_API_ISSUE, ApplicationCommandExt, MessageExt},
    Args, BotResult, CommandData, Context,
};

use rosu_v2::prelude::{
    Beatmapset, BeatmapsetSearchResult, BeatmapsetSearchSort, GameMode, Genre, Language, Osu,
    OsuResult, RankStatus,
};
use std::{collections::BTreeMap, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Search for mapsets")]
#[long_desc(
    "Search for mapsets. \n\
    The query works essentially the same as in game, meaning you can add \
    any keywords, aswell as specific assignments like `creator=abc`, `length<123`, `ar>=9`, ...\n\n\
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
    To reverse, specify `-asc`."
)]
#[aliases("searchmap", "mapsearch")]
#[usage("[search query]")]
#[example(
    "some words yay mode=osu status=graveyard sort=favourites -asc",
    "artist=camellia length<240 stars>8 genre=electronic"
)]
async fn search(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match MapSearchArgs::args(&mut args) {
            Ok(search_args) => {
                _search(ctx, CommandData::Message { msg, args, num }, search_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_mapsearch(ctx, *command).await,
    }
}

async fn _search(ctx: Arc<Context>, data: CommandData<'_>, args: MapSearchArgs) -> BotResult<()> {
    let mut search_result = match args.request(ctx.osu()).await {
        Ok(response) => response,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE);

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let mapset_count = search_result.mapsets.len();
    let total_pages = (mapset_count < 50).then(|| mapset_count / 10 + 1);
    let maps: BTreeMap<usize, Beatmapset> = search_result.mapsets.drain(..).enumerate().collect();
    let embed_data = MapSearchEmbed::new(&maps, &args, (1, total_pages)).await;

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let response_raw = data.create_message(&ctx, embed.into()).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let owner = data.author()?.id;
    let response = response_raw.model().await?;

    // Pagination
    let pagination =
        MapSearchPagination::new(Arc::clone(&ctx), response, maps, search_result, args);

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mapsearch): {}")
        }
    });

    Ok(())
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
    pub fn args(args: &mut Args) -> Result<Self, &'static str> {
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
                        let msg = "Failed to parse `mode`. After `mode=` you must \
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
                        let msg = "Failed to parse `status`. After `status=` you must \
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
                        let msg = "Failed to parse `language`. After `language=` you must \
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
                        let msg = "Failed to parse `video` boolean. After `video=` \
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
                        let msg = "Failed to parse `storyboard` boolean. After `storyboard=` \
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
                        let msg = "Failed to parse `nsfw` boolean. After `nsfw=` \
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
                        let msg = "Failed to parse `sort`. After `sort=` you must \
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

    pub async fn request(&self, osu: &Osu) -> OsuResult<BeatmapsetSearchResult> {
        let mut search_fut = osu
            .beatmapset_search()
            .video(self.video)
            .storyboard(self.storyboard)
            .nsfw(self.nsfw)
            .sort(self.sort, self.descending);

        if let Some(ref query) = self.query {
            search_fut = search_fut.query(query);
        }

        if let Some(mode) = self.mode {
            search_fut = search_fut.mode(mode);
        }

        if let Some(ref status) = self.status {
            search_fut = match status.status() {
                Some(status) => search_fut.status(status),
                None => search_fut.any_status(),
            };
        }

        if let Some(genre) = self.genre {
            search_fut = search_fut.genre(genre);
        }

        if let Some(language) = self.language {
            search_fut = search_fut.language(language);
        }

        search_fut.await
    }
}

pub async fn slash_mapsearch(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut query = None;
    let mut mode = None;
    let mut status = None;
    let mut genre = None;
    let mut language = None;
    let mut video = None;
    let mut storyboard = None;
    let mut nsfw = None;
    let mut sort = None;
    let mut descending = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "query" => query = Some(value),
                "mode" => mode = parse_mode_option!(value, "search"),
                "status" => match value.as_str() {
                    "any" => status = Some(SearchRankStatus(None)),
                    "leaderboard" => status = None,
                    "ranked" => status = Some(SearchRankStatus(Some(RankStatus::Ranked))),
                    "loved" => status = Some(SearchRankStatus(Some(RankStatus::Loved))),
                    "qualified" => status = Some(SearchRankStatus(Some(RankStatus::Qualified))),
                    "pending" => status = Some(SearchRankStatus(Some(RankStatus::Pending))),
                    "graveyard" => status = Some(SearchRankStatus(Some(RankStatus::Graveyard))),
                    _ => bail_cmd_option!("search status", string, value),
                },
                "genre" => match value.as_str() {
                    "any" => genre = Some(Genre::Any),
                    "anime" => genre = Some(Genre::Anime),
                    "classical" => genre = Some(Genre::Classical),
                    "electronic" => genre = Some(Genre::Electronic),
                    "folk" => genre = Some(Genre::Folk),
                    "hiphop" => genre = Some(Genre::HipHop),
                    "jazz" => genre = Some(Genre::Jazz),
                    "metal" => genre = Some(Genre::Metal),
                    "novelty" => genre = Some(Genre::Novelty),
                    "other" => genre = Some(Genre::Other),
                    "pop" => genre = Some(Genre::Pop),
                    "rock" => genre = Some(Genre::Rock),
                    "unspecified" => genre = Some(Genre::Unspecified),
                    "videogame" => genre = Some(Genre::VideoGame),
                    _ => bail_cmd_option!("search genre", string, value),
                },
                "language" => match value.as_str() {
                    "any" => language = Some(Language::Any),
                    "chinese" => language = Some(Language::Chinese),
                    "english" => language = Some(Language::English),
                    "french" => language = Some(Language::French),
                    "german" => language = Some(Language::German),
                    "instrumental" => language = Some(Language::Instrumental),
                    "italian" => language = Some(Language::Italian),
                    "japanese" => language = Some(Language::Japanese),
                    "korean" => language = Some(Language::Korean),
                    "other" => language = Some(Language::Other),
                    "polish" => language = Some(Language::Polish),
                    "russian" => language = Some(Language::Russian),
                    "spanish" => language = Some(Language::Spanish),
                    "swedish" => language = Some(Language::Swedish),
                    "unspecified" => language = Some(Language::Unspecified),
                    _ => bail_cmd_option!("search language", string, value),
                },
                "sort" => match value.as_str() {
                    "artist" => sort = Some(BeatmapsetSearchSort::Artist),
                    "favourites" => sort = Some(BeatmapsetSearchSort::Favourites),
                    "playcount" => sort = Some(BeatmapsetSearchSort::Playcount),
                    "rankeddate" => sort = Some(BeatmapsetSearchSort::RankedDate),
                    "rating" => sort = Some(BeatmapsetSearchSort::Rating),
                    "relevance" => sort = Some(BeatmapsetSearchSort::Relevance),
                    "stars" => sort = Some(BeatmapsetSearchSort::Stars),
                    "title" => sort = Some(BeatmapsetSearchSort::Title),
                    _ => bail_cmd_option!("search sort", string, value),
                },
                _ => bail_cmd_option!("search", string, name),
            },
            CommandDataOption::Integer { name, .. } => bail_cmd_option!("search", integer, name),
            CommandDataOption::Boolean { name, value } => match name.as_str() {
                "video" => video = Some(value),
                "storyboard" => storyboard = Some(value),
                "nsfw" => nsfw = Some(value),
                "reverse" => descending = Some(!value),
                _ => bail_cmd_option!("search", boolean, name),
            },
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("search", subcommand, name)
            }
        }
    }

    let args = MapSearchArgs {
        query,
        mode,
        status,
        genre,
        language,
        video: video.unwrap_or(false),
        storyboard: storyboard.unwrap_or(false),
        nsfw: nsfw.unwrap_or(true),
        sort: sort.unwrap_or(BeatmapsetSearchSort::Relevance),
        descending: descending.unwrap_or(true),
    };

    _search(ctx, command.into(), args).await
}

// TODO: Add user search
pub fn slash_mapsearch_command() -> Command {
    let options = vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify a search query".to_owned(),
            name: "query".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: super::mode_choices(),
            description: "Specify a mode".to_owned(),
            name: "mode".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "any".to_owned(),
                    value: "any".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "leaderboard".to_owned(),
                    value: "leaderboard".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "ranked".to_owned(),
                    value: "ranked".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "loved".to_owned(),
                    value: "loved".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "qualified".to_owned(),
                    value: "qualified".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "pending".to_owned(),
                    value: "pending".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "graveyard".to_owned(),
                    value: "graveyard".to_owned(),
                },
            ],
            description: "Specify a ranking status".to_owned(),
            name: "status".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "any".to_owned(),
                    value: "any".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "anime".to_owned(),
                    value: "anime".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "classical".to_owned(),
                    value: "classical".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "electronic".to_owned(),
                    value: "electronic".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "folk".to_owned(),
                    value: "folk".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "hiphop".to_owned(),
                    value: "hiphop".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "jazz".to_owned(),
                    value: "jazz".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "metal".to_owned(),
                    value: "metal".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "novelty".to_owned(),
                    value: "novelty".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "other".to_owned(),
                    value: "other".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "pop".to_owned(),
                    value: "pop".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "rock".to_owned(),
                    value: "rock".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "unspecified".to_owned(),
                    value: "unspecified".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "videogame".to_owned(),
                    value: "videogame".to_owned(),
                },
            ],
            description: "Specify a genre".to_owned(),
            name: "genre".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "any".to_owned(),
                    value: "any".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "chinese".to_owned(),
                    value: "chinese".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "english".to_owned(),
                    value: "english".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "french".to_owned(),
                    value: "french".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "german".to_owned(),
                    value: "german".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "instrumental".to_owned(),
                    value: "instrumental".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "italian".to_owned(),
                    value: "italian".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "japanese".to_owned(),
                    value: "japanese".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "korean".to_owned(),
                    value: "korean".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "other".to_owned(),
                    value: "other".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "polish".to_owned(),
                    value: "polish".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "russian".to_owned(),
                    value: "russian".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "spanish".to_owned(),
                    value: "spanish".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "swedish".to_owned(),
                    value: "swedish".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "unspecified".to_owned(),
                    value: "unspecified".to_owned(),
                },
            ],
            description: "Specify a language".to_owned(),
            name: "language".to_owned(),
            required: false,
        }),
        CommandOption::Boolean(BaseCommandOptionData {
            description: "Specify if the mapset should have a video".to_owned(),
            name: "video".to_owned(),
            required: false,
        }),
        CommandOption::Boolean(BaseCommandOptionData {
            description: "Specify if the mapset should have a video".to_owned(),
            name: "storyboard".to_owned(),
            required: false,
        }),
        CommandOption::Boolean(BaseCommandOptionData {
            description: "Specify whether the mapset can be NSFW".to_owned(),
            name: "nsfw".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "artist".to_owned(),
                    value: "artist".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "favourites".to_owned(),
                    value: "favourites".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "playcount".to_owned(),
                    value: "playcount".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "rankeddata".to_owned(),
                    value: "rankeddata".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "rating".to_owned(),
                    value: "rating".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "relevance".to_owned(),
                    value: "relevance".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "stars".to_owned(),
                    value: "stars".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "title".to_owned(),
                    value: "title".to_owned(),
                },
            ],
            description: "Specify the order of mapsets".to_owned(),
            name: "sort".to_owned(),
            required: false,
        }),
        CommandOption::Boolean(BaseCommandOptionData {
            description: "Specify whether the resulting list should be reversed".to_owned(),
            name: "reverse".to_owned(),
            required: false,
        }),
    ];

    SlashCommandBuilder::new("search", "Search for mapsets")
        .options(options)
        .build()
}
