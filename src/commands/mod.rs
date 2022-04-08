use rosu_v2::prelude::{GameMode, Grade};
use twilight_interactions::command::{CommandOption, CreateOption};

use self::osu::GradeArg;

pub mod fun;
pub mod help;
pub mod osu;
pub mod owner;
pub mod songs;
pub mod tracking;
pub mod twitch;
pub mod utility;

#[derive(CommandOption, CreateOption)]
pub enum ShowHideOption {
    #[option(name = "Show", value = "show")]
    Show,
    #[option(name = "Hide", value = "hide")]
    Hide,
}

#[derive(CommandOption, CreateOption)]
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
    fn from(mode: GameModeOption) -> Self {
        match mode {
            GameModeOption::Osu => Self::STD,
            GameModeOption::Taiko => Self::TKO,
            GameModeOption::Catch => Self::CTB,
            GameModeOption::Mania => Self::MNA,
        }
    }
}

#[derive(CommandOption, CreateOption)]
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

impl From<GradeOption> for GradeArg {
    fn from(grade: GradeOption) -> Self {
        match grade {
            SS => Self::Single(Grade::SS),
            S => Self::Single(Grade::S),
            A => Self::Single(Grade::A),
            B => Self::Single(Grade::B),
            C => Self::Single(Grade::C),
            D => Self::Single(Grade::D),
            F => Self::Single(Grade::F),
        }
    }
}
