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

// discord
pub const DISCORD_CDN: &str = "https://cdn.discordapp.com/";

// Discord ids
pub const OWNER_USER_ID: u64 = 219905108316520448;
#[allow(dead_code)]
pub const DEV_GUILD_ID: u64 = 297072529426612224;

// Error messages
pub const GENERAL_ISSUE: &str = "Something went wrong, blame bade";
pub const OSU_API_ISSUE: &str = "Some issue with the osu api, blame bade";
pub const OSU_WEB_ISSUE: &str = "Some issue with the osu website, DDoS protection?";

// Misc
pub const SYMBOLS: [&str; 6] = ["♔", "♕", "♖", "♗", "♘", "♙"];
pub const DATE_FORMAT: &str = "%F %T";
pub const INVITE_LINK: &str = "https://discordapp.com/api/oauth2/authorize?scope=bot&\
    client_id=297073686916366336&permissions=268823616";
