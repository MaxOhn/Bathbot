#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Site {
    DiscordAttachment,
    Huismetbenen,
    MissAnalyzer,
    Osekai,
    OsuAvatar,
    OsuBadge,
    OsuHiddenApi,
    OsuMapFile,
    OsuMapsetCover,
    OsuStats,
    OsuTracker,
    Respektive,
    Twitch,
}

impl Site {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DiscordAttachment => "DiscordAttachment",
            Self::Huismetbenen => "Huismetbenen",
            Self::MissAnalyzer => "MissAnalyzer",
            Self::Osekai => "Osekai",
            Self::OsuAvatar => "OsuAvatar",
            Self::OsuBadge => "OsuBadge",
            Self::OsuHiddenApi => "OsuHiddenApi",
            Self::OsuMapFile => "OsuMapFile",
            Self::OsuMapsetCover => "OsuMapsetCover",
            Self::OsuStats => "OsuStats",
            Self::OsuTracker => "OsuTracker",
            Self::Respektive => "Respektive",
            Self::Twitch => "Twitch",
        }
    }
}
