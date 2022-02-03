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
            UserStatsColumn::Badges => "badges",
            UserStatsColumn::Comments => "comment_count",
            UserStatsColumn::Followers => "followers",
            UserStatsColumn::ForumPosts => "forum_post_count",
            UserStatsColumn::GraveyardMapsets => "graveyard_mapset_count",
            UserStatsColumn::JoinDate => "join_date",
            UserStatsColumn::KudosuAvailable => "kudosu_available",
            UserStatsColumn::KudosuTotal => "kudosu_total",
            UserStatsColumn::LovedMapsets => "loved_mapset_count",
            UserStatsColumn::MappingFollowers => "mapping_followers",
            UserStatsColumn::Medals => "medals",
            UserStatsColumn::PlayedMaps => "played_maps",
            UserStatsColumn::RankedMapsets => "ranked_mapset_count",
            UserStatsColumn::Usernames => "previous_usernames_count",

            UserStatsColumn::Accuracy { .. } => "accuracy",
            UserStatsColumn::CountSsh { .. } => "count_ssh",
            UserStatsColumn::CountSs { .. } => "count_ss",
            UserStatsColumn::CountSh { .. } => "count_sh",
            UserStatsColumn::CountS { .. } => "count_s",
            UserStatsColumn::CountA { .. } => "count_a",
            UserStatsColumn::Level { .. } => "level",
            UserStatsColumn::MaxCombo { .. } => "max_combo",
            UserStatsColumn::Playcount { .. } => "playcount",
            UserStatsColumn::Playtime { .. } => "playtime",
            UserStatsColumn::Pp { .. } => "pp",
            UserStatsColumn::RankCountry { .. } => "country_rank",
            UserStatsColumn::RankGlobal { .. } => "global_rank",
            UserStatsColumn::Replays { .. } => "replays_watched",
            UserStatsColumn::ScoreRanked { .. } => "ranked_score",
            UserStatsColumn::ScoreTotal { .. } => "total_score",
            UserStatsColumn::ScoresFirst { .. } => "scores_first",
            UserStatsColumn::TotalHits { .. } => "total_hits",
        }
    }
}

pub struct UserValueRaw<T> {
    pub username: Username,
    pub country_code: CountryCode,
    pub value: T,
}
