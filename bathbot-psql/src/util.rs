use rosu_v2::prelude::{GameMode, Genre, Grade, Language, RankStatus};

pub fn parse_mode(mode: i16) -> GameMode {
    match mode {
        0 => GameMode::Osu,
        1 => GameMode::Taiko,
        2 => GameMode::Catch,
        3 => GameMode::Mania,
        _ => unreachable!(),
    }
}

pub fn parse_grade(grade: i16) -> Grade {
    match grade {
        0 => Grade::F,
        1 => Grade::D,
        2 => Grade::C,
        3 => Grade::B,
        4 => Grade::A,
        5 => Grade::S,
        6 => Grade::SH,
        7 => Grade::X,
        8 => Grade::XH,
        _ => unreachable!(),
    }
}

pub fn parse_status(status: i16) -> RankStatus {
    match status {
        -2 => RankStatus::Graveyard,
        -1 => RankStatus::WIP,
        0 => RankStatus::Pending,
        1 => RankStatus::Ranked,
        2 => RankStatus::Approved,
        3 => RankStatus::Qualified,
        4 => RankStatus::Loved,
        _ => unreachable!(),
    }
}

pub fn parse_genre(genre: i16) -> Genre {
    match genre {
        0 => Genre::Any,
        1 => Genre::Unspecified,
        2 => Genre::VideoGame,
        3 => Genre::Anime,
        4 => Genre::Rock,
        5 => Genre::Pop,
        6 => Genre::Other,
        7 => Genre::Novelty,
        9 => Genre::HipHop,
        10 => Genre::Electronic,
        11 => Genre::Metal,
        12 => Genre::Classical,
        13 => Genre::Folk,
        14 => Genre::Jazz,
        _ => unreachable!(),
    }
}

pub fn parse_language(language: i16) -> Language {
    match language {
        0 => Language::Any,
        1 => Language::Other,
        2 => Language::English,
        3 => Language::Japanese,
        4 => Language::Chinese,
        5 => Language::Instrumental,
        6 => Language::Korean,
        7 => Language::French,
        8 => Language::German,
        9 => Language::Swedish,
        10 => Language::Spanish,
        11 => Language::Italian,
        12 => Language::Russian,
        13 => Language::Polish,
        14 => Language::Unspecified,
        _ => unreachable!(),
    }
}
