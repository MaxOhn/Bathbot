use std::fmt;

use http::StatusCode;
use serde_json::Error;

#[derive(Debug, thiserror::Error)]
pub enum CustomClientError {
    #[error("failed to create header value")]
    InvalidHeader(#[from] hyper::header::InvalidHeaderValue),
    #[error("http error")]
    Http(#[from] hyper::http::Error),
    #[error("hyper error")]
    Hyper(#[from] hyper::Error),
    #[error("reached retry limit and still failed to download {0}.osu")]
    MapFileRetryLimit(u32),
    #[cfg(debug_assertions)]
    #[error("don't make twitch requests on debug")]
    NoTwitchOnDebug,
    #[error("timeout while waiting for osu stats")]
    OsuStatsTimeout,
    #[error("could not deserialize {kind}: {body}")]
    Parsing {
        body: String,
        kind: ErrorKind,
        #[source]
        source: Error,
    },
    #[error("failed to serialize")]
    Serialize(#[source] serde_json::Error),
    #[error("failed with status code {status} when requesting {url}")]
    Status { status: StatusCode, url: String },
    #[error("failed to serialize url encoding")]
    UrlEncoded(#[from] serde_urlencoded::ser::Error),
}

impl CustomClientError {
    pub fn parsing(source: Error, bytes: &[u8], kind: ErrorKind) -> Self {
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
    OsekaiBadgeOwners,
    OsekaiBadges,
    OsekaiComments,
    OsekaiMaps,
    OsekaiMedals,
    OsekaiRanking(&'static str),
    OsuStatsGlobal,
    OsuStatsGlobalAmount,
    OsuStatsGlobalScores,
    OsuTrackerCountryDetails,
    OsuTrackerIdCounts,
    OsuTrackerPpGroup,
    OsuTrackerStats,
    RankData,
    RespektiveRank,
    RespektiveUser,
    SnipeCountry,
    SnipePlayer,
    SnipeRecent,
    SnipeScore,
    SnipeScoreCount,
    TwitchStreams,
    TwitchToken,
    TwitchUserId,
    TwitchUserName,
    TwitchUsers,
    TwitchVideos,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::CountryStatistics => "country statistics",
            Self::GlobalsList => "globals list",
            Self::Leaderboard => "leaderboard",
            Self::OsekaiBadgeOwners => "osekai badge owners",
            Self::OsekaiBadges => "osekai badges",
            Self::OsekaiComments => "osekai comments",
            Self::OsekaiMaps => "osekai maps",
            Self::OsekaiMedals => "osekai medals",
            Self::OsekaiRanking(ranking) => ranking,
            Self::OsuStatsGlobal => "osu stats global",
            Self::OsuStatsGlobalAmount => "osu stats global amount",
            Self::OsuStatsGlobalScores => "osu stats global scores",
            Self::OsuTrackerCountryDetails => "osu tracker country details",
            Self::OsuTrackerIdCounts => "osu tracker id counts",
            Self::OsuTrackerPpGroup => "osu tracker pp group",
            Self::OsuTrackerStats => "osu tracker stats",
            Self::RankData => "rank data",
            Self::RespektiveRank => "respektive rank",
            Self::RespektiveUser => "respektive user",
            Self::SnipeCountry => "snipe country",
            Self::SnipePlayer => "snipe player",
            Self::SnipeRecent => "snipe recent",
            Self::SnipeScore => "snipe score",
            Self::SnipeScoreCount => "snipe score count",
            Self::TwitchStreams => "twitch streams",
            Self::TwitchToken => "twitch token",
            Self::TwitchUserId => "twitch user id",
            Self::TwitchUserName => "twitch user name",
            Self::TwitchUsers => "twitch users",
            Self::TwitchVideos => "twitch videos",
        };

        f.write_str(kind)
    }
}
