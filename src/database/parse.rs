use rosu::models::{ApprovalStatus, GameMode, Genre, Language};

pub fn mode_to_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "osu",
        GameMode::TKO => "taiko",
        GameMode::CTB => "fruits",
        GameMode::MNA => "mania",
    }
}

pub fn str_to_mode(mode: &str) -> GameMode {
    match mode {
        "osu" => GameMode::STD,
        "taiko" => GameMode::TKO,
        "fruits" => GameMode::CTB,
        "mania" => GameMode::MNA,
        _ => panic!("Can not parse '{}' into mode", mode),
    }
}
