mod map_multi;
mod nochoke;
mod pp_missing;
mod profile;
mod score_multi;
mod score_single;
mod simulate;
mod whatif_pp;

pub use map_multi::MapMultiData;
pub use nochoke::NoChokeData;
pub use pp_missing::PPMissingData;
pub use profile::ProfileData;
pub use score_multi::ScoreMultiData;
pub use score_single::ScoreSingleData;
pub use simulate::SimulateData;
pub use whatif_pp::WhatIfPPData;

pub const HOMEPAGE: &str = "https://osu.ppy.sh/";
pub const MAP_THUMB_URL: &str = "https://b.ppy.sh/thumb/";
pub const AVATAR_URL: &str = "https://a.ppy.sh/";
pub const FLAG_URL: &str = "https://osu.ppy.sh//images/flags/";
