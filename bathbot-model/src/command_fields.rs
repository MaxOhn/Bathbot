use std::str::FromStr;

use rosu_v2::prelude::{GameMode, Grade};
use time::UtcOffset;
use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum ShowHideOption {
    #[option(name = "Show", value = "show")]
    Show = 0,
    #[option(name = "Hide", value = "hide")]
    Hide = 1,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
pub enum EnableDisable {
    #[option(name = "Enable", value = "enable")]
    Enable,
    #[option(name = "Disable", value = "disable")]
    Disable,
}

#[derive(CommandOption, CreateOption)]
pub enum ThreadChannel {
    #[option(name = "Stay in channel", value = "channel")]
    Channel,
    #[option(name = "Start new thread", value = "thread")]
    Thread,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum GameModeOption {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
    #[option(name = "mania", value = "mania")]
    Mania,
}

impl From<GameModeOption> for GameMode {
    #[inline]
    fn from(mode: GameModeOption) -> Self {
        match mode {
            GameModeOption::Osu => Self::Osu,
            GameModeOption::Taiko => Self::Taiko,
            GameModeOption::Catch => Self::Catch,
            GameModeOption::Mania => Self::Mania,
        }
    }
}

impl From<GameMode> for GameModeOption {
    #[inline]
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::Osu => Self::Osu,
            GameMode::Taiko => Self::Taiko,
            GameMode::Catch => Self::Catch,
            GameMode::Mania => Self::Mania,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum GradeOption {
    #[option(name = "SS", value = "ss")]
    SS,
    #[option(name = "S", value = "s")]
    S,
    #[option(name = "A", value = "a")]
    A,
    #[option(name = "B", value = "b")]
    B,
    #[option(name = "C", value = "c")]
    C,
    #[option(name = "D", value = "d")]
    D,
    #[option(name = "F", value = "f")]
    F,
}

impl From<GradeOption> for Grade {
    #[inline]
    fn from(grade: GradeOption) -> Self {
        match grade {
            GradeOption::SS => Self::X,
            GradeOption::S => Self::S,
            GradeOption::A => Self::A,
            GradeOption::B => Self::B,
            GradeOption::C => Self::C,
            GradeOption::D => Self::D,
            GradeOption::F => Self::F,
        }
    }
}

impl FromStr for GradeOption {
    type Err = &'static str;

    // ! Make sure the given strings are lower case
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let grade = match s {
            "x" | "ss" => Self::SS,
            "s" => Self::S,
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "f" => Self::F,
            _ => {
                return Err("Failed to parse `grade`.\n\
                Valid grades are: `SS`, `S`, `A`, `B`, `C`, `D`, or `F`");
            }
        };

        Ok(grade)
    }
}

macro_rules! timezone_option {
    ( $( $variant:ident, $name:literal, $value:literal, $value_str:literal; )* ) => {
        #[derive(CommandOption, CreateOption)]
        pub enum TimezoneOption {
            $(
                #[option(name = $name, value = $value_str)]
                $variant = $value,
            )*
        }

        impl From<TimezoneOption> for UtcOffset {
            #[inline]
            fn from(tz: TimezoneOption) -> Self {
                let seconds = match tz {
                    $(
                        #[allow(clippy::neg_multiply, clippy::erasing_op, clippy::identity_op)]
                        TimezoneOption:: $variant => $value * 3600,
                    )*
                };

                Self::from_whole_seconds(seconds).unwrap()
            }
        }
    }
}

timezone_option! {
    M12, "UTC-12", -12, "-12";
    M11, "UTC-11", -11, "-11";
    M10, "UTC-10", -10, "-10";
    M9, "UTC-9", -9, "-9";
    M8, "UTC-8", -8, "-8";
    M7, "UTC-7", -7, "-7";
    M6, "UTC-6", -6, "-6";
    M5, "UTC-5", -5, "-5";
    M4, "UTC-4", -4, "-4";
    M3, "UTC-3", -3, "-3";
    M2, "UTC-2", -2, "-2";
    M1, "UTC-1", -1, "-1";
    P0, "UTC+0", 0, "0";
    P1, "UTC+1", 1, "1";
    P2, "UTC+2", 2, "2";
    P3, "UTC+3", 3, "3";
    P4, "UTC+4", 4, "4";
    P5, "UTC+5", 5, "5";
    P6, "UTC+6", 6, "6";
    P7, "UTC+7", 7, "7";
    P8, "UTC+8", 8, "8";
    P9, "UTC+9", 9, "9";
    P10, "UTC+10", 10, "10";
    P11, "UTC+11", 11, "11";
    P12, "UTC+12", 12, "12";
}
