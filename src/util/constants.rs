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
pub const OSU_DAILY_API: &str = "https://osudaily.net/api/";

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
pub const OSU_DAILY_ISSUE: &str = "Some issue with the osudaily api, blame bade";
pub const OSUSTATS_API_ISSUE: &str = "Some issue with the osustats api, blame bade";
pub const OSUTRACKER_ISSUE: &str = "Some issue with the osutracker api, blame bade";
pub const TWITCH_API_ISSUE: &str = "Some issue with the twitch api, blame bade";

// Discord error codes
pub const MESSAGE_TOO_OLD_TO_BULK_DELETE: u64 = 50034;
pub const UNKNOWN_CHANNEL: u64 = 10003;

// Misc
pub const OWNER_USER_ID: u64 = 219905108316520448;
pub const SYMBOLS: [&str; 6] = ["♔", "♕", "♖", "♗", "♘", "♙"];
pub const DATE_FORMAT: &str = "%F %T";
pub const INVITE_LINK: &str = "https://discord.com/api/oauth2/authorize?client_id=297073686916366336&permissions=36776045632&scope=bot%20applications.commands";
pub const BATHBOT_WORKSHOP: &str = "https://discord.gg/n9fFstG";
pub const BATHBOT_GITHUB: &str = "https://github.com/MaxOhn/Bathbot";
pub const BATHBOT_WORKSHOP_ID: u64 = 741040473476694159;

pub mod common_literals {
    pub const HELP: &str = "help";
    pub const MODE: &str = "mode";
    pub const NAME: &str = "name";
    pub const DISCORD: &str = "discord";
    pub const INDEX: &str = "index";
    pub const GRADE: &str = "grade";
    pub const MODS: &str = "mods";
    pub const MAP: &str = "map";
    pub const COUNTRY: &str = "country";
    pub const REVERSE: &str = "reverse";
    pub const SORT: &str = "sort";
    pub const SCORE: &str = "score";
    pub const COMBO: &str = "combo";
    pub const RANK: &str = "rank";
    pub const ACC: &str = "acc";
    pub const ACCURACY: &str = "accuracy";
    pub const MISSES: &str = "misses";
    pub const PROFILE: &str = "profile";
    pub const USERNAME: &str = "username";
    pub const USER_ID: &str = "user_id";

    pub const OSU: &str = "osu";
    pub const TAIKO: &str = "taiko";
    pub const CTB: &str = "ctb";
    pub const FRUITS: &str = "fruits";
    pub const MANIA: &str = "mania";

    pub const SPECIFY_MODE: &str = "Specify a gamemode";
    pub const SPECIFY_COUNTRY: &str = "Specify a country (code)";
    pub const CONSIDER_GRADE: &str = "Consider only scores with this grade";

    pub const MODS_PARSE_FAIL: &str =
        "Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.";
    pub const MAP_PARSE_FAIL: &str =
        "Failed to parse map url. Be sure you specify a valid map id or url to a map.";
}
