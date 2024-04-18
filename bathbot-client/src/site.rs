#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Site {
    DiscordAttachment,
    Flags,
    Github,
    Huismetbenen,
    KittenRoleplay,
    MissAnalyzer,
    Osekai,
    OsuAvatar,
    OsuBadge,
    OsuMapFile,
    OsuMapsetCover,
    OsuReplay,
    OsuStats,
    OsuTrack,
    OsuTracker,
    OsuWorld,
    Respektive,
    Twitch,
}

impl Site {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DiscordAttachment => "DiscordAttachment",
            Self::Flags => "Flag",
            Self::Github => "Github",
            Self::Huismetbenen => "Huismetbenen",
            Self::KittenRoleplay => "KittenRoleplay",
            Self::MissAnalyzer => "MissAnalyzer",
            Self::Osekai => "Osekai",
            Self::OsuAvatar => "OsuAvatar",
            Self::OsuBadge => "OsuBadge",
            Self::OsuMapFile => "OsuMapFile",
            Self::OsuMapsetCover => "OsuMapsetCover",
            Self::OsuReplay => "OsuReplay",
            Self::OsuStats => "OsuStats",
            Self::OsuTrack => "OsuTrack",
            Self::OsuTracker => "OsuTracker",
            Self::OsuWorld => "OsuWorld",
            Self::Respektive => "Respektive",
            Self::Twitch => "Twitch",
        }
    }
}
