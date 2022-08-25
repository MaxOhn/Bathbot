use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult, Write},
    mem,
};

use rosu_v2::prelude::{
    GameMode, Grade, MatchEvent, MatchGame, MatchScore, OsuMatch, ScoringType, TeamType,
    UserCompact, Username,
};
use smallvec::SmallVec;
use twilight_model::channel::embed::Embed;

use crate::util::{
    builder::{EmbedBuilder, FooterBuilder},
    constants::{DESCRIPTION_SIZE, OSU_BASE},
    datetime::sec_to_minsec,
    numbers::{round, with_comma_int},
    osu::grade_emote,
    CowUtils, Emote, ScoreExt,
};

const DESCRIPTION_BUFFER: usize = 45;

pub type MatchLiveEmbeds = SmallVec<[MatchLiveEmbed; 2]>;

pub struct MatchLiveEmbed {
    title: String,
    url: String,
    description: String,
    image: Option<String>,
    footer: Option<FooterBuilder>,
    state: Option<GameState>,
}

#[derive(Copy, Clone, Debug)]
struct GameState {
    game_id: u64,
    finished: bool,
}

macro_rules! push {
    ($buf:expr => $content:literal @ $lobby:ident[$user_id:ident]) => {
        writeln!($buf, $content, username!($lobby[$user_id])).unwrap()
    };
}

macro_rules! username {
    ($lobby:ident[$user_id:ident]) => {
        match $lobby.users.get(&$user_id) {
            Some(user) => Cow::Borrowed(user.username.as_str()),
            None => format!("User id {}", $user_id).into(),
        }
    };
}

macro_rules! image {
    ($mapset:ident) => {
        format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            $mapset.mapset_id
        )
    };
}

macro_rules! team {
    ($team:ident,$scores:ident -> $buf:ident) => {
        if $team == 1 {
            $buf.push_str(":blue_circle: **Blue Team** :blue_circle:");

            if let Some((score, _)) = $scores {
                let _ = write!($buf, " | {}", with_comma_int(score));
            }

            $buf.push('\n');
        } else if $team == 2 {
            $buf.push_str(":red_circle: **Red Team** :red_circle:");

            if let Some((_, score)) = $scores {
                let _ = write!($buf, " | {}", with_comma_int(score));
            }

            $buf.push('\n');
        }
    };
}

impl MatchLiveEmbed {
    pub fn new(lobby: &OsuMatch) -> MatchLiveEmbeds {
        let mut embeds = MatchLiveEmbeds::new();

        if lobby.events.is_empty() {
            return embeds;
        }

        let mut description = String::new();
        let mut state: Option<GameState>;

        for i in 0..lobby.events.len() {
            // SAFETY: i is guaranteed to be within bounds
            let event = unsafe { lobby.events.get_unchecked(i) };
            state = None;

            match event {
                MatchEvent::Joined { user_id, .. } => {
                    push!(description => "• `{}` joined the lobby" @ lobby[user_id])
                }
                MatchEvent::Left { user_id, .. } => {
                    push!(description => "• `{}` left the lobby" @ lobby[user_id])
                }
                MatchEvent::Create { user_id, .. } => match *user_id {
                    Some(user_id) => {
                        push!(description => "• `{}` created the lobby" @ lobby[user_id])
                    }
                    None => description.push_str("• The lobby has been created"),
                },
                MatchEvent::HostChanged { user_id, .. } => {
                    push!(description => "• `{}` became the new host" @ lobby[user_id])
                }
                MatchEvent::Kicked { user_id, .. } => {
                    push!(description => "• `{}` kicked from the lobby" @ lobby[user_id])
                }
                MatchEvent::Disbanded { .. } => description.push_str("• **Lobby was closed**"),
                MatchEvent::Game { game, .. } => {
                    let next_state = GameState {
                        game_id: game.game_id,
                        finished: game.end_time.is_some(),
                    };

                    // Finish up the embed we have so far
                    if !description.is_empty() {
                        let embed = Self {
                            title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                            url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                            description,
                            image: None,
                            footer: None,
                            state: None,
                        };

                        embeds.push(embed);
                        description = String::new();
                    } else if let Some(state) = state {
                        if !state.finished && next_state.finished {
                            embeds.pop();
                        }
                    }

                    let (description, image, footer) = game_content(lobby, game);
                    state = Some(next_state);

                    let embed = Self {
                        title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                        url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                        description,
                        image,
                        footer,
                        state,
                    };

                    embeds.push(embed);

                    // If the game is on-going and has no following game event, return early
                    if game.end_time.is_none() {
                        let last_game = lobby.events.get(i + 1..).map_or(true, |events| {
                            events.iter().all(|e| !matches!(e, MatchEvent::Game { .. }))
                        });

                        if last_game {
                            return embeds;
                        } else {
                            embeds.pop();
                        }
                    }
                }
            }

            if description.len() + DESCRIPTION_BUFFER > DESCRIPTION_SIZE {
                let embed = Self {
                    title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                    url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                    description,
                    image: None,
                    footer: None,
                    state: None,
                };

                embeds.push(embed);
                description = String::new();
            }
        }

        if !description.is_empty() {
            let embed = Self {
                title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                description,
                image: None,
                footer: None,
                state: None,
            };

            embeds.push(embed);
        }

        embeds
    }

    pub fn update(&mut self, lobby: &OsuMatch) -> (bool, Option<MatchLiveEmbeds>) {
        if lobby.events.is_empty() {
            return (false, None);
        }

        let mut update = None;
        let mut embeds = MatchLiveEmbeds::new();
        let mut last_state = self.state;

        for i in 0..lobby.events.len() {
            // SAFETY: i is guaranteed to be within bounds
            let event = unsafe { lobby.events.get_unchecked(i) };

            // The previous embed was a game
            if let Some(state) = last_state.take() {
                let mut embed = Self {
                    title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                    url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                    description: String::new(),
                    image: None,
                    footer: None,
                    state: None,
                };

                match event {
                    MatchEvent::Joined { user_id, .. } => {
                        push!(embed.description => "• `{}` joined the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Left { user_id, .. } => {
                        push!(embed.description => "• `{}` left the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Kicked { user_id, .. } => {
                        push!(embed.description => "• `{}` kicked from the lobby" @ lobby[user_id])
                    }
                    MatchEvent::HostChanged { user_id, .. } => {
                        push!(embed.description => "• `{}` became the new host" @ lobby[user_id])
                    }
                    MatchEvent::Create { user_id, .. } => match *user_id {
                        Some(user_id) => {
                            push!(embed.description => "• `{}` created the lobby" @ lobby[user_id])
                        }
                        None => embed.description.push_str("• The lobby has been created"),
                    },
                    MatchEvent::Disbanded { .. } => {
                        embed.description.push_str("• **Lobby was closed**")
                    }
                    MatchEvent::Game { game, .. } => {
                        let curr_state = GameState {
                            game_id: game.game_id,
                            finished: game.end_time.is_some(),
                        };

                        last_state = Some(curr_state);

                        if state.game_id == curr_state.game_id && !curr_state.finished {
                            update.get_or_insert(false);

                            // If the game is on-going and has no following game event, return early
                            let last_game = lobby.events.get(i + 1..).map_or(true, |events| {
                                events.iter().all(|e| !matches!(e, MatchEvent::Game { .. }))
                            });

                            if last_game {
                                return (false, (!embeds.is_empty()).then(|| embeds));
                            }

                            continue;
                        }

                        let (description, image, footer) = game_content(lobby, game);

                        // Previous game not yet finished but next one already there => override
                        if !state.finished {
                            let (mut embed, empty) = match embeds.last_mut() {
                                Some(embed) => (embed, false),
                                None => (&mut *self, true),
                            };

                            embed.description = description;
                            embed.image = image;
                            embed.footer = footer;
                            embed.state = last_state;

                            update.get_or_insert(empty);
                        } else {
                            // Different game, can be either finished or not
                            embed.description = description;
                            embed.image = image;
                            embed.footer = footer;
                            embed.state = last_state;

                            // If the game is on-going and has no following game event, return early
                            if game.end_time.is_none() {
                                let last_game = lobby.events.get(i + 1..).map_or(true, |events| {
                                    events.iter().all(|e| !matches!(e, MatchEvent::Game { .. }))
                                });

                                if last_game {
                                    embeds.push(embed);

                                    return (update.unwrap_or(false), Some(embeds));
                                }
                            }
                        }
                    }
                }

                update.get_or_insert(false);

                match embeds.last_mut().filter(|e| e.description.is_empty()) {
                    Some(last) => mem::swap(last, &mut embed),
                    None if !embed.description.is_empty() => embeds.push(embed),
                    _ => {}
                }

            // The previous embed was not a game
            } else {
                let (mut embed, empty) = match embeds.last_mut() {
                    Some(embed) => (embed, false),
                    None => (&mut *self, true),
                };

                match event {
                    MatchEvent::Joined { user_id, .. } => {
                        update.get_or_insert(empty);

                        push!(embed.description => "• `{}` joined the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Left { user_id, .. } => {
                        update.get_or_insert(empty);

                        push!(embed.description => "• `{}` left the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Kicked { user_id, .. } => {
                        update.get_or_insert(empty);

                        push!(embed.description => "• `{}` kicked from the lobby" @ lobby[user_id])
                    }
                    MatchEvent::HostChanged { user_id, .. } => {
                        update.get_or_insert(empty);

                        push!(embed.description => "• `{}` became the new host" @ lobby[user_id])
                    }
                    MatchEvent::Create { user_id, .. } => {
                        update.get_or_insert(empty);

                        match *user_id {
                            Some(user_id) => {
                                push!(embed.description => "• `{}` created the lobby" @ lobby[user_id])
                            }
                            None => embed.description.push_str("• The lobby has been created"),
                        }
                    }
                    MatchEvent::Disbanded { .. } => {
                        update.get_or_insert(empty);

                        embed.description.push_str("• **Lobby was closed**")
                    }
                    MatchEvent::Game { game, .. } => {
                        let (description, image, footer) = game_content(lobby, game);

                        let state = GameState {
                            game_id: game.game_id,
                            finished: game.end_time.is_some(),
                        };

                        last_state = Some(state);

                        if embed.description.is_empty() {
                            embed.description = description;
                            embed.image = image;
                            embed.footer = footer;
                            embed.state = last_state;
                        } else {
                            let new_embed = Self {
                                title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                                url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                                description,
                                image,
                                footer,
                                state: last_state,
                            };

                            embeds.push(new_embed);

                            // If the game is on-going and has no following game event, return early
                            if game.end_time.is_none() {
                                let last_game = lobby.events.get(i + 1..).map_or(true, |events| {
                                    events.iter().all(|e| !matches!(e, MatchEvent::Game { .. }))
                                });

                                if last_game {
                                    return (update.unwrap_or(false), Some(embeds));
                                }
                            }

                            embed = embeds.last_mut().unwrap();
                        }
                    }
                }

                if embed.description.len() + DESCRIPTION_BUFFER > DESCRIPTION_SIZE
                    && i != lobby.events.len() - 1
                {
                    let embed = Self {
                        title: lobby.name.as_str().cow_escape_markdown().into_owned(),
                        url: format!("{OSU_BASE}community/matches/{}", lobby.match_id),
                        description: String::new(),
                        image: None,
                        footer: None,
                        state: None,
                    };

                    embeds.push(embed);
                }
            }
        }

        (
            update.unwrap_or(false),
            (!embeds.is_empty()).then(|| embeds),
        )
    }

    pub fn as_embed(&self) -> Embed {
        let mut builder = EmbedBuilder::new()
            .description(&self.description)
            .title(&self.title)
            .url(&self.url);

        builder = if let Some(ref image) = self.image {
            builder.image(image)
        } else {
            builder
        };

        if let Some(ref footer) = self.footer {
            builder.footer(footer).build()
        } else {
            builder.build()
        }
    }
}

/// Return the description and image for a either in-progress or finished games
fn game_content(
    lobby: &OsuMatch,
    game: &MatchGame,
) -> (String, Option<String>, Option<FooterBuilder>) {
    let mut description = String::with_capacity(128);

    match game.end_time {
        Some(_) => {
            let image = match game.map {
                Some(ref map) => {
                    let mapset = map.mapset.as_ref().unwrap();

                    let _ = write!(
                        description,
                        "**[{artist} - {title} [{version}]]({OSU_BASE}b/{map_id})",
                        artist = mapset.artist.cow_escape_markdown(),
                        title = mapset.title.cow_escape_markdown(),
                        version = map.version.cow_escape_markdown(),
                        map_id = map.map_id,
                    );

                    if !game.mods.is_empty() {
                        let _ = write!(description, " +{}", game.mods);
                    }

                    Some(image!(mapset))
                }
                None => {
                    description.push_str("**Unknown map");

                    if !game.mods.is_empty() {
                        let _ = write!(description, " +{}", game.mods);
                    }

                    None
                }
            };

            description.push_str("**\n\n");

            let (scores, sizes, team_scores) =
                prepare_scores(game.mode, &game.scores, &lobby.users, game.scoring_type);

            let mut team = match scores.first().map(|s| s.team) {
                Some(team) => team,
                None => {
                    description.push_str("Game aborted");

                    return (description, image, None);
                }
            };

            if matches!(game.team_type, TeamType::TeamVS | TeamType::TagTeamVS) {
                team!(team,team_scores -> description);
            }

            for score in scores {
                if score.team != team
                    && matches!(game.team_type, TeamType::TeamVS | TeamType::TagTeamVS)
                {
                    team = score.team;
                    description.push('\n');

                    team!(team,team_scores -> description);
                }

                let _ = writeln!(
                    description,
                    "{grade} `{name:<len$}` `+{mods:<mods_len$}` `{acc:>5}%` `{combo:>combo_len$}x` `{score:>score_len$}`{miss}",
                    grade = grade_emote(score.grade),
                    name = score.username,
                    len = sizes.name,
                    mods = score.mods,
                    mods_len = sizes.mods,
                    acc = round(score.accuracy),
                    combo = score.combo,
                    combo_len = sizes.combo,
                    score = score.score_str,
                    score_len = sizes.score,
                    miss = MissFormat(score.count_miss),
                );
            }

            let footer = team_scores.map(|(blue, red)| {
                let difference = (blue as i64 - red as i64).abs();

                let footer = match blue.cmp(&red) {
                    Ordering::Greater => {
                        format!("Blue Team wins by {}", with_comma_int(difference as u64))
                    }
                    Ordering::Less => {
                        format!("Red Team wins by {}", with_comma_int(difference as u64))
                    }
                    Ordering::Equal => "Team scores are tied".to_owned(),
                };

                FooterBuilder::new(footer)
            });

            (description, image, footer)
        }
        None => {
            let image = match game.map {
                Some(ref map) => {
                    let mapset = map.mapset.as_ref().unwrap();

                    let _ = write!(
                        description,
                        "**[{artist} - {title} [{version}]]({OSU_BASE}b/{map_id})",
                        artist = mapset.artist.cow_escape_markdown(),
                        title = mapset.title.cow_escape_markdown(),
                        version = map.version.cow_escape_markdown(),
                        map_id = map.map_id,
                    );

                    if !game.mods.is_empty() {
                        let _ = write!(description, " +{}", game.mods);
                    }

                    let _ = write!(
                        description,
                        "**\nLength: `{}`",
                        sec_to_minsec(map.seconds_total)
                    );

                    Some(image!(mapset))
                }
                None => {
                    description.push_str("**Unknown map");

                    if !game.mods.is_empty() {
                        let _ = write!(description, " +{}", game.mods);
                    }

                    description.push_str("**\n");

                    None
                }
            };

            let _ = write!(
                description,
                " | {:?} | {:?}",
                game.scoring_type, game.team_type
            );

            (description, image, None)
        }
    }
}

type Scores = SmallVec<[EmbedScore; 16]>;

#[derive(Default)]
struct ColumnSizes {
    name: usize,
    combo: usize,
    score: usize,
    mods: usize,
}

enum TeamLeads {
    Score([u64; 3]),
    Acc([f32; 3]),
    Combo([u64; 3]),
}

impl TeamLeads {
    fn new(scoring: ScoringType) -> Self {
        match scoring {
            ScoringType::ScoreV2 | ScoringType::Score => Self::Score([0; 3]),
            ScoringType::Accuracy => Self::Acc([0.0; 3]),
            ScoringType::Combo => Self::Combo([0; 3]),
        }
    }

    fn update(&mut self, score: &MatchScore) {
        match self {
            Self::Score(arr) => arr[score.team as usize] += score.score as u64,
            Self::Acc(arr) => {
                arr[score.team as usize] = arr[score.team as usize].max(score.accuracy)
            }
            Self::Combo(arr) => {
                arr[score.team as usize] = arr[score.team as usize].max(score.max_combo as u64)
            }
        }
    }

    fn finish(self) -> (TeamValues, Option<(u64, u64)>) {
        match self {
            Self::Score(arr) => {
                let team_scores =
                    (arr[0] == 0 && (arr[1] > 0 || arr[2] > 0)).then(|| (arr[1], arr[2]));

                (TeamValues::U64(arr), team_scores)
            }
            Self::Acc(arr) => (TeamValues::Float(arr), None),
            Self::Combo(arr) => (TeamValues::U64(arr), None),
        }
    }
}

enum TeamValues {
    U64([u64; 3]),
    Float([f32; 3]),
}

fn prepare_scores(
    mode: GameMode,
    scores: &[MatchScore],
    users: &HashMap<u32, UserCompact>,
    scoring: ScoringType,
) -> (Scores, ColumnSizes, Option<(u64, u64)>) {
    let mut embed_scores = Scores::with_capacity(users.len());
    let mut sizes = ColumnSizes::default();
    let mut team_scores = TeamLeads::new(scoring);

    let iter = scores.iter().filter(|score| score.score > 0).map(|score| {
        let name: Username = match users.get(&score.user_id) {
            Some(user) => user.username.as_str().into(),
            None => format!("`User id {}`", score.user_id).into(),
        };

        let score_str = with_comma_int(score.score).to_string();
        let combo = with_comma_int(score.max_combo).to_string();
        let mods = score.mods.to_string();
        let team = score.team as usize;
        let grade = score.grade(mode);

        sizes.name = sizes.name.max(name.len());
        sizes.combo = sizes.combo.max(combo.len());
        sizes.score = sizes.score.max(score_str.len());
        sizes.mods = sizes.mods.max(mods.len());

        team_scores.update(score);

        EmbedScore {
            username: name,
            mods,
            accuracy: score.accuracy,
            team,
            grade,
            combo,
            score: score.score,
            score_str,
            count_miss: score.statistics.count_miss,
        }
    });

    embed_scores.extend(iter);

    let scores = match team_scores.finish() {
        (TeamValues::U64(arr), scores) => {
            embed_scores.sort_unstable_by(|s1, s2| {
                arr[s2.team]
                    .cmp(&arr[s1.team])
                    .then_with(|| s2.score.cmp(&s1.score))
            });

            scores
        }
        (TeamValues::Float(arr), _) => {
            embed_scores.sort_unstable_by(|s1, s2| {
                arr[s2.team]
                    .partial_cmp(&arr[s1.team])
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| s2.score.cmp(&s1.score))
            });

            None
        }
    };

    (embed_scores, sizes, scores)
}

struct EmbedScore {
    username: Username,
    mods: String,
    accuracy: f32,
    team: usize,
    grade: Grade,
    combo: String,
    score: u32,
    score_str: String,
    count_miss: u32,
}

struct MissFormat(u32);

impl Display for MissFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(
            f,
            " {miss}{emote}",
            miss = self.0,
            emote = Emote::Miss.text()
        )
    }
}
