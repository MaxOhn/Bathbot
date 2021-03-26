use crate::{
    embeds::{EmbedBuilder, EmbedData},
    util::{
        constants::{DESCRIPTION_SIZE, MAP_THUMB_URL, OSU_BASE},
        datetime::sec_to_minsec,
        numbers::{round, with_comma_uint},
    },
    Name,
};

use rosu_v2::prelude::{
    GameMods, MatchEvent, MatchGame, MatchScore, OsuMatch, ScoringType, TeamType, UserCompact,
};
use smallvec::SmallVec;
use std::{borrow::Cow, cmp::Ordering, fmt::Write};

const DESCRIPTION_BUFFER: usize = 45;

type MatchLiveEmbeds = SmallVec<[MatchLiveEmbed; 2]>;

pub struct MatchLiveEmbed {
    title: String,
    url: String,
    description: String,
    image: Option<String>,
    game_id: Option<u64>,
}

macro_rules! push {
    ($buf:expr => $content:literal @ $lobby:ident[$user_id:ident]) => {
        writeln!($buf, $content, username!($lobby[$user_id])).unwrap()
    };
}

macro_rules! username {
    ($lobby:ident[$user_id:ident]) => {
        match $lobby.users.iter().find(|user| &user.user_id == $user_id) {
            Some(user) => Cow::Borrowed(&user.username),
            None => Cow::Owned(format!("User id {}", $user_id)),
        }
    };
}

macro_rules! image {
    ($mapset:ident) => {
        format!("{}{}l.jpg", MAP_THUMB_URL, $mapset.mapset_id)
    };
}

macro_rules! team {
    ($team:ident -> $buf:ident) => {
        if $team == 1 {
            let _ = write!($buf, ":blue_circle: **Blue Team** :blue_circle:");
        } else if $team == 2 {
            let _ = write!($buf, ":red_circle: **Red Team** :red_circle:");
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

        for event in &lobby.events {
            match event {
                MatchEvent::Joined { user_id, .. } => {
                    push!(description => "• `{}` joined the lobby" @ lobby[user_id])
                }
                MatchEvent::Left { user_id, .. } => {
                    push!(description => "• `{}` left the lobby" @ lobby[user_id])
                }
                MatchEvent::Create { user_id, .. } => {
                    push!(description => "• `{}` created the lobby" @ lobby[user_id])
                }
                MatchEvent::HostChanged { user_id, .. } => {
                    push!(description => "• `{}` became the new host" @ lobby[user_id])
                }
                MatchEvent::Kicked { user_id, .. } => {
                    push!(description => "• `{}` kicked from the lobby" @ lobby[user_id])
                }
                MatchEvent::Disbanded { .. } => description.push_str("Lobby was closed"),
                MatchEvent::Game { game, .. } => {
                    description.clear();
                    let (description, image) = game_content(lobby, &*game);

                    let embed = Self {
                        title: lobby.name.to_owned(),
                        url: format!("{}community/matches/{}", OSU_BASE, lobby.match_id),
                        description,
                        image,
                        game_id: Some(game.game_id),
                    };

                    embeds.push(embed);
                }
            }

            if description.len() + DESCRIPTION_BUFFER > DESCRIPTION_SIZE {
                let embed = Self {
                    title: lobby.name.to_owned(),
                    url: format!("{}community/matches/{}", OSU_BASE, lobby.match_id),
                    description,
                    image: None,
                    game_id: None,
                };

                embeds.push(embed);
                description = String::new();
            }
        }

        if !description.is_empty() {
            let embed = Self {
                title: lobby.name.to_owned(),
                url: format!("{}community/matches/{}", OSU_BASE, lobby.match_id),
                description,
                image: None,
                game_id: None,
            };

            embeds.push(embed);
        }

        embeds
    }

    pub fn update(&mut self, lobby: &OsuMatch) -> (bool, Option<MatchLiveEmbeds>) {
        if lobby.events.is_empty() {
            return (false, None);
        }

        let mut updated = false;
        let mut embeds = MatchLiveEmbeds::new();
        let mut last_game_id = self.game_id;

        let mut events = lobby.events.iter();

        while let Some(event) = events.next() {
            let mut next_game_id = None;
            std::mem::swap(&mut next_game_id, &mut last_game_id);

            // New embed, except if event is game with same id and still in progress
            if let Some(game_id) = next_game_id {
                let mut embed = Self {
                    title: lobby.name.to_owned(),
                    url: format!("{}community/matches/{}", OSU_BASE, lobby.match_id),
                    description: String::new(),
                    image: None,
                    game_id: None,
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
                    MatchEvent::Create { user_id, .. } => {
                        push!(embed.description => "• `{}` created the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Disbanded { .. } => {
                        embed.description.push_str("Lobby was closed\n")
                    }
                    MatchEvent::Game { game, .. } => {
                        last_game_id = Some(game.game_id);

                        if game.end_time.is_none() && game.game_id == game_id {
                            continue;
                        }

                        let (description, image) = game_content(lobby, &*game);

                        embed.description = description;
                        embed.image = image;
                        embed.game_id = Some(game.game_id);
                    }
                }

                match embeds.last_mut().filter(|e| e.description.is_empty()) {
                    Some(last) => std::mem::swap(last, &mut embed),
                    None => embeds.push(embed),
                };
            // Extend existing embed unless its a game event
            } else {
                let (mut embed, empty) = match embeds.last_mut() {
                    Some(embed) => (embed, false),
                    None => (&mut *self, true),
                };

                if !embed.description.is_empty() {
                    embed.description.push('\n');
                }

                match event {
                    MatchEvent::Joined { user_id, .. } => {
                        updated |= empty;

                        push!(embed.description => "• `{}` joined the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Left { user_id, .. } => {
                        updated |= empty;

                        push!(embed.description => "• `{}` left the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Kicked { user_id, .. } => {
                        updated |= empty;

                        push!(embed.description => "• `{}` kicked from the lobby" @ lobby[user_id])
                    }
                    MatchEvent::HostChanged { user_id, .. } => {
                        updated |= empty;

                        push!(embed.description => "• `{}` became the new host" @ lobby[user_id])
                    }
                    MatchEvent::Create { user_id, .. } => {
                        updated |= empty;

                        push!(embed.description => "• `{}` created the lobby" @ lobby[user_id])
                    }
                    MatchEvent::Disbanded { .. } => {
                        updated |= empty;

                        embed.description.push_str("Lobby was closed\n")
                    }
                    MatchEvent::Game { game, .. } => {
                        let (description, image) = game_content(lobby, &*game);

                        if embed.description.is_empty() {
                            embed.description = description;
                            embed.image = image;
                            embed.game_id = Some(game.game_id);
                        } else {
                            let new_embed = Self {
                                title: lobby.name.to_owned(),
                                url: format!("{}community/matches/{}", OSU_BASE, lobby.match_id),
                                description,
                                image,
                                game_id: Some(game.game_id),
                            };

                            embeds.push(new_embed);
                            embed = embeds.last_mut().unwrap();
                        }

                        last_game_id = Some(game.game_id);
                    }
                }

                if embed.description.len() + DESCRIPTION_BUFFER > DESCRIPTION_SIZE {
                    let (remaining, _) = events.size_hint();

                    if remaining > 0 {
                        let embed = Self {
                            title: lobby.name.to_owned(),
                            url: format!("{}community/matches/{}", OSU_BASE, lobby.match_id),
                            description: String::new(),
                            image: None,
                            game_id: None,
                        };

                        embeds.push(embed);
                    }
                }
            }
        }

        (updated, (!embeds.is_empty()).then(|| embeds))
    }
}

impl EmbedData for MatchLiveEmbed {
    fn as_builder(&self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .description(&self.description)
            .title(&self.title)
            .url(&self.url);

        if let Some(ref image) = self.image {
            builder.image(image.to_owned())
        } else {
            builder
        }
    }
}

/// Return the description and image for a either in-progress or finished games
fn game_content(lobby: &OsuMatch, game: &MatchGame) -> (String, Option<String>) {
    let mut description = String::with_capacity(128);

    match game.end_time {
        Some(_) => {
            let image = match game.map {
                Some(ref map) => {
                    let mapset = map.mapset.as_ref().unwrap();

                    let _ = write!(
                        description,
                        "**[{artist} - {title} [{version}]]({base}b/{map_id})",
                        artist = mapset.artist,
                        title = mapset.title,
                        version = map.version,
                        base = OSU_BASE,
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

            let (scores, sizes) = prepare_scores(&game.scores, &lobby.users, game.scoring_type);

            let mut team = scores.first().unwrap().team;

            if matches!(game.team_type, TeamType::TeamVS | TeamType::TagTeamVS) {
                team!(team -> description);
            }

            for score in scores {
                if score.team != team
                    && matches!(game.team_type, TeamType::TeamVS | TeamType::TagTeamVS)
                {
                    team = score.team;

                    team!(team -> description);
                }

                let _ = write!(
                    description,
                    "`{name:<len$}`",
                    name = score.username,
                    len = sizes.name
                );

                if !score.mods.is_empty() {
                    let _ = write!(description, " +{}", game.mods);
                }

                let _ = writeln!(
                    description,
                    " `{acc:>5}%` `{combo:>combo_len$}x` `{score:>score_len$}`",
                    acc = round(score.accuracy),
                    combo = score.combo,
                    combo_len = sizes.combo,
                    score = score.score_str,
                    score_len = sizes.score,
                );
            }

            (description, image)
        }
        None => {
            let image = match game.map {
                Some(ref map) => {
                    let mapset = map.mapset.as_ref().unwrap();

                    let _ = write!(
                        description,
                        "**[{artist} - {title} [{version}]]({base}b/{map_id})",
                        artist = mapset.artist,
                        title = mapset.title,
                        version = map.version,
                        base = OSU_BASE,
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

                    // TODO: Add more data

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
                " [{:?} | {:?}]",
                game.scoring_type, game.team_type
            );

            (description, image)
        }
    }
}

type Scores = SmallVec<[EmbedScore; 16]>;

#[derive(Default)]
struct ColumnSizes {
    name: usize,
    combo: usize,
    score: usize,
}

enum TeamLeads {
    Score([u32; 3]),
    Acc([f32; 3]),
    Combo([u32; 3]),
}

impl TeamLeads {
    #[inline]
    fn new(scoring: ScoringType) -> Self {
        match scoring {
            ScoringType::ScoreV2 | ScoringType::Score => Self::Score([0; 3]),
            ScoringType::Accuracy => Self::Acc([0.0; 3]),
            ScoringType::Combo => Self::Combo([0; 3]),
        }
    }

    #[inline]
    fn update(&mut self, score: &MatchScore) {
        match self {
            Self::Score(arr) => arr[score.team as usize] += score.score,
            Self::Acc(arr) => {
                arr[score.team as usize] = arr[score.team as usize].max(score.accuracy)
            }
            Self::Combo(arr) => {
                arr[score.team as usize] = arr[score.team as usize].max(score.max_combo)
            }
        }
    }

    #[inline]
    fn finish(self) -> TeamValues {
        match self {
            Self::Score(arr) => TeamValues::U32(arr),
            Self::Acc(arr) => TeamValues::Float(arr),
            Self::Combo(arr) => TeamValues::U32(arr),
        }
    }
}

enum TeamValues {
    U32([u32; 3]),
    Float([f32; 3]),
}

fn prepare_scores(
    scores: &[MatchScore],
    users: &[UserCompact],
    scoring: ScoringType,
) -> (Scores, ColumnSizes) {
    let mut embed_scores = Scores::with_capacity(users.len());
    let mut sizes = ColumnSizes::default();
    let mut team_scores = TeamLeads::new(scoring);

    let iter = scores.iter().filter(|score| score.score > 0).map(|score| {
        let user_opt = users.iter().find(|user| user.user_id == score.user_id);

        let name: Name = match user_opt {
            Some(user) => user.username.as_str().into(),
            None => format!("`User id {}`", score.user_id).into(),
        };

        let score_str = with_comma_uint(score.score).to_string();
        let combo = with_comma_uint(score.max_combo).to_string();
        let team = score.team as usize;

        sizes.name = sizes.name.max(name.len());
        sizes.combo = sizes.combo.max(combo.len());
        sizes.score = sizes.score.max(score_str.len());

        team_scores.update(score);

        EmbedScore {
            username: name,
            mods: score.mods,
            accuracy: score.accuracy,
            team,
            combo,
            score: score.score,
            score_str,
        }
    });

    embed_scores.extend(iter);

    match team_scores.finish() {
        TeamValues::U32(arr) => {
            embed_scores.sort_unstable_by(|s1, s2| {
                arr[s2.team]
                    .cmp(&arr[s1.team])
                    .then_with(|| s2.score.cmp(&s1.score))
            });
        }
        TeamValues::Float(arr) => {
            embed_scores.sort_unstable_by(|s1, s2| {
                arr[s2.team]
                    .partial_cmp(&arr[s1.team])
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| s2.score.cmp(&s1.score))
            });
        }
    }

    (embed_scores, sizes)
}

struct EmbedScore {
    username: Name,
    mods: GameMods,
    accuracy: f32,
    team: usize,
    combo: String,
    score: u32,
    score_str: String,
}
