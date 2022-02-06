use rosu_v2::prelude::{GameMode, Username};

use crate::util::CountryCode;

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum UserStatsColumn {
    Badges,
    Comments,
    Followers,
    ForumPosts,
    GraveyardMapsets,
    JoinDate,
    #[allow(dead_code)]
    KudosuAvailable,
    #[allow(dead_code)]
    KudosuTotal,
    LovedMapsets,
    MappingFollowers,
    Medals,
    PlayedMaps,
    RankedMapsets,
    Usernames,

    Accuracy { mode: GameMode },
    AverageHits { mode: GameMode },
    CountSsh { mode: GameMode },
    CountSs { mode: GameMode },
    CountSh { mode: GameMode },
    CountS { mode: GameMode },
    CountA { mode: GameMode },
    Level { mode: GameMode },
    MaxCombo { mode: GameMode },
    Playcount { mode: GameMode },
    Playtime { mode: GameMode },
    Pp { mode: GameMode },
    RankCountry { mode: GameMode },
    RankGlobal { mode: GameMode },
    Replays { mode: GameMode },
    ScoreRanked { mode: GameMode },
    ScoreTotal { mode: GameMode },
    ScoresFirst { mode: GameMode },
    TotalHits { mode: GameMode },
}

impl UserStatsColumn {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Badges => "badges",
            Self::Comments => "comment_count",
            Self::Followers => "followers",
            Self::ForumPosts => "forum_post_count",
            Self::GraveyardMapsets => "graveyard_mapset_count",
            Self::JoinDate => "join_date",
            Self::KudosuAvailable => "kudosu_available",
            Self::KudosuTotal => "kudosu_total",
            Self::LovedMapsets => "loved_mapset_count",
            Self::MappingFollowers => "mapping_followers",
            Self::Medals => "medals",
            Self::PlayedMaps => "played_maps",
            Self::RankedMapsets => "ranked_mapset_count",
            Self::Usernames => "previous_usernames_count",

            Self::Accuracy { .. } => "accuracy",
            Self::AverageHits { .. } => "", // handled manually
            Self::CountSsh { .. } => "count_ssh",
            Self::CountSs { .. } => "count_ss",
            Self::CountSh { .. } => "count_sh",
            Self::CountS { .. } => "count_s",
            Self::CountA { .. } => "count_a",
            Self::Level { .. } => "level",
            Self::MaxCombo { .. } => "max_combo",
            Self::Playcount { .. } => "playcount",
            Self::Playtime { .. } => "playtime",
            Self::Pp { .. } => "pp",
            Self::RankCountry { .. } => "country_rank",
            Self::RankGlobal { .. } => "global_rank",
            Self::Replays { .. } => "replays_watched",
            Self::ScoreRanked { .. } => "ranked_score",
            Self::ScoreTotal { .. } => "total_score",
            Self::ScoresFirst { .. } => "scores_first",
            Self::TotalHits { .. } => "total_hits",
        }
    }
}

pub struct UserValueRaw<T> {
    pub username: Username,
    pub country_code: CountryCode,
    pub value: T,
}
