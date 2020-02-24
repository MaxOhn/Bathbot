mod auth_desc_thumb;
mod auth_desc_thumb_title;
mod common;
mod leaderboard;
mod profile;
mod score_multi;
mod score_single;
mod simulate;

pub use auth_desc_thumb::AuthorDescThumbData;
pub use auth_desc_thumb_title::AuthorDescThumbTitleData;
pub use common::CommonData;
pub use leaderboard::LeaderboardData;
pub use profile::ProfileData;
pub use score_multi::ScoreMultiData;
pub use score_single::ScoreSingleData;
pub use simulate::SimulateData;

pub const HOMEPAGE: &str = "https://osu.ppy.sh/";
pub const MAP_THUMB_URL: &str = "https://b.ppy.sh/thumb/";
pub const AVATAR_URL: &str = "https://a.ppy.sh/";
pub const FLAG_URL: &str = "https://osu.ppy.sh//images/flags/";
