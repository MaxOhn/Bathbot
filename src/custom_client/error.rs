use std::fmt;

use serde_json::Error;

#[derive(Debug, thiserror::Error)]
pub enum CustomClientError {
    #[error("could not deserialize {kind}: {body}")]
    Parsing {
        body: String,
        kind: ErrorKind,
        #[source]
        source: Error,
    },
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("timeout while waiting for osu stats")]
    OsuStatsTimeout,
}

impl CustomClientError {
    pub(super) fn parsing(source: Error, bytes: &[u8], kind: ErrorKind) -> Self {
        Self::Parsing {
            body: String::from_utf8_lossy(bytes).into_owned(),
            source,
            kind,
        }
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    CountryStatistics,
    GlobalsList,
    Leaderboard,
    OsekaiComments,
    OsekaiMaps,
    OsekaiMedals,
    OsekaiRanking(&'static str),
    OsuStatsGlobal,
    OsuStatsGlobalAmount,
    OsuStatsGlobalScores,
    RankData,
    SnipeCountry,
    SnipePlayer,
    SnipeRecent,
    SnipeScore,
    SnipeScoreCount,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::CountryStatistics => "country statistics",
            Self::GlobalsList => "globals list",
            Self::Leaderboard => "leaderboard",
            Self::OsekaiComments => "osekai comments",
            Self::OsekaiMaps => "osekai maps",
            Self::OsekaiMedals => "osekai medals",
            Self::OsekaiRanking(ranking) => ranking,
            Self::OsuStatsGlobal => "osu stats global",
            Self::OsuStatsGlobalAmount => "osu stats global amount",
            Self::OsuStatsGlobalScores => "osu stats global scores",
            Self::RankData => "rank data",
            Self::SnipeCountry => "snipe country",
            Self::SnipePlayer => "snipe player",
            Self::SnipeRecent => "snipe recent",
            Self::SnipeScore => "snipe score",
            Self::SnipeScoreCount => "snipe score count",
        };

        f.write_str(kind)
    }
}
