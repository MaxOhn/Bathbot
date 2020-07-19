// Colors
pub const DARK_GREEN: u32 = 0x1F8B4C;
pub const RED: u32 = 0xE74C3C;

// Message field sizes
pub const EMBED_SIZE: usize = 6000;
pub const DESCRIPTION_SIZE: usize = 2048;
pub const FIELD_VALUE_SIZE: usize = 1024;

// osu!
pub const OSU_BASE: &str = "https://osu.ppy.sh/";
pub const MAP_THUMB_URL: &str = "https://b.ppy.sh/thumb/";
pub const AVATAR_URL: &str = "https://a.ppy.sh/";

// twitch
pub const TWITCH_BASE: &str = "https://www.twitch.tv/";
pub const TWITCH_STREAM_ENDPOINT: &str = "https://api.twitch.tv/helix/streams";
pub const TWITCH_USERS_ENDPOINT: &str = "https://api.twitch.tv/helix/users";

// Discord ids
pub const OWNER_USER_ID: u64 = 219905108316520448;
pub const DEV_GUILD_ID: u64 = 297072529426612224;

// Error messages
pub const GENERAL_ISSUE: &str = "Something went wrong, blame bade";
pub const OSU_API_ISSUE: &str = "Some issue with the osu api, blame bade";

// Misc
pub const SYMBOLS: [&str; 6] = ["♔", "♕", "♖", "♗", "♘", "♙"];
pub const DATE_FORMAT: &str = "%F %T";

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
