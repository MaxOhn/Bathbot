#![allow(clippy::unreadable_literal)]

pub mod emotes {
    pub const EMOTE_XH_ID: u64 = 515354675059621888;
    pub const EMOTE_X_ID: u64 = 515354674929336320;
    pub const EMOTE_SH_ID: u64 = 515354675323600933;
    pub const EMOTE_S_ID: u64 = 515354674791186433;
    pub const EMOTE_A_ID: u64 = 515339175222837259;
    pub const EMOTE_B_ID: u64 = 515354674866683904;
    pub const EMOTE_C_ID: u64 = 515354674476351492;
    pub const EMOTE_D_ID: u64 = 515354674963021824;
    pub const EMOTE_F_ID: u64 = 515623098947600385;
}

pub const DEV_GUILD_ID: u64 = 297072529426612224;
pub const MAIN_GUILD_ID: u64 = 277469642908237826; // also ChannelId of #general
pub const WELCOME_CHANNEL: u64 = 438410203977744394;
pub const UNCHECKED_ROLE_ID: u64 = 326390404620746752;
pub const TOP_ROLE_ID: u64 = 438450781142908929;

pub const MSG_MEMORY: usize = 1000;

pub const GENERAL_ISSUE: &str = "Something went wrong, blame bade";
pub const OSU_API_ISSUE: &str = "Some issue with the osu api, blame bade";

pub const DATE_FORMAT: &str = "%F %T";

pub const MINIMIZE_DELAY: i64 = 45;

pub const PP_MANIA_CMD: &str =
    "dotnet run --project osu-tools/PerformanceCalculator/ -- simulate mania ";

pub const AUTHORITY_ROLES: &str = "admin mod moderator";

pub const HOMEPAGE: &str = "https://osu.ppy.sh/";
pub const MAP_THUMB_URL: &str = "https://b.ppy.sh/thumb/";
pub const AVATAR_URL: &str = "https://a.ppy.sh/";

pub const TWITCH_BASE: &str = "https://www.twitch.tv/";
pub const TWITCH_STREAM_ENDPOINT: &str = "https://api.twitch.tv/helix/streams";
pub const TWITCH_USERS_ENDPOINT: &str = "https://api.twitch.tv/helix/users";
