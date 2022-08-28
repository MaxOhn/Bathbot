// Colors
pub const DARK_GREEN: u32 = 0x1F8B4C;
pub const RED: u32 = 0xE74C3C;

// Message field sizes
pub const DESCRIPTION_SIZE: usize = 4096;
pub const FIELD_VALUE_SIZE: usize = 1024;

// osu!
pub const OSU_BASE: &str = "https://osu.ppy.sh/";
pub const MAP_THUMB_URL: &str = "https://b.ppy.sh/thumb/";
pub const AVATAR_URL: &str = "https://a.ppy.sh/";
pub const HUISMETBENEN: &str = "https://api.huismetbenen.nl/";

// twitch
pub const TWITCH_BASE: &str = "https://www.twitch.tv/";
pub const TWITCH_OAUTH: &str = "https://id.twitch.tv/oauth2/token";
pub const TWITCH_STREAM_ENDPOINT: &str = "https://api.twitch.tv/helix/streams";
pub const TWITCH_USERS_ENDPOINT: &str = "https://api.twitch.tv/helix/users";
pub const TWITCH_VIDEOS_ENDPOINT: &str = "https://api.twitch.tv/helix/videos";

// Error messages
pub const GENERAL_ISSUE: &str = "Something went wrong, blame bade";
pub const OSU_API_ISSUE: &str = "Some issue with the osu api, blame bade";
pub const OSU_WEB_ISSUE: &str = "Some issue with the osu website, DDoS protection?";
pub const OSEKAI_ISSUE: &str = "Some issue with the osekai api, blame bade";
pub const HUISMETBENEN_ISSUE: &str = "Some issue with the huismetbenen api, blame bade";
pub const OSUSTATS_API_ISSUE: &str = "Some issue with the osustats api, blame bade";
pub const OSUTRACKER_ISSUE: &str = "Some issue with the osutracker api, blame bade";
pub const TWITCH_API_ISSUE: &str = "Some issue with the twitch api, blame bade";
pub const THREADS_UNAVAILABLE: &str = "Cannot start new thread from here";

// Discord error codes
pub const INVALID_ACTION_FOR_CHANNEL_TYPE: u64 = 50024;
pub const MESSAGE_TOO_OLD_TO_BULK_DELETE: u64 = 50034;
pub const UNKNOWN_CHANNEL: u64 = 10003;

// Misc
pub const SYMBOLS: [&str; 6] = ["♔", "♕", "♖", "♗", "♘", "♙"];
pub const INVITE_LINK: &str = "https://discord.com/api/oauth2/authorize?client_id=297073686916366336&permissions=36776045632&scope=bot%20applications.commands";
pub const BATHBOT_WORKSHOP: &str = "https://discord.gg/n9fFstG";
pub const BATHBOT_GITHUB: &str = "https://github.com/MaxOhn/Bathbot";
pub const BATHBOT_ROADMAP: &str = "https://github.com/MaxOhn/Bathbot/projects/1";
pub const KOFI: &str = "https://ko-fi.com/bathbot";
