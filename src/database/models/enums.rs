use bytes::BytesMut;
use rosu::models::{ApprovalStatus, GameMode, Genre, Language};
use std::error::Error;
use tokio_postgres::types::{FromSql, IsNull, Kind, ToSql, Type};

type TResult<T> = std::result::Result<T, Box<dyn Error + Sync + Send>>;

impl<'a> FromSql<'a> for GameMode {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> TResult<Self> {
        let enums = match *ty.kind() {
            Kind::Enum(ref enums) => enums,
            _ => panic!("expected enum type"),
        };
        for e in enums.iter() {
            if e.as_bytes() == raw {
                let mode = match e {
                    "osu" => GameMode::STD,
                    "taiko" => GameMode::TKO,
                    "fruits" => GameMode::CTB,
                    "mania" => GameMode::MNA,
                    _ => panic!("expected gamemode enum, got {}", e),
                };
                return Ok(mode);
            }
        }
        panic!("did not match any enum");
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 4,
            _ => false,
        }
    }
}

impl ToSql for GameMode {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> TResult<IsNull> {
        let mode = match self {
            GameMode::STD => "osu",
            GameMode::TKO => "taiko",
            GameMode::CTB => "fruits",
            GameMode::MNA => "mania",
        };
        mode.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 4,
            _ => false,
        }
    }
}

impl<'a> FromSql<'a> for ApprovalStatus {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> TResult<Self> {
        let enums = match *ty.kind() {
            Kind::Enum(ref enums) => enums,
            _ => panic!("expected enum type"),
        };
        for e in enums.iter() {
            if e.as_bytes() == raw {
                let mode = match e {
                    "loved" => ApprovalStatus::Loved,
                    "qualified" => ApprovalStatus::Qualified,
                    "approved" => ApprovalStatus::Approved,
                    "ranked" => ApprovalStatus::Ranked,
                    "pending" => ApprovalStatus::Pending,
                    "wip" => ApprovalStatus::WIP,
                    "graveyard" => ApprovalStatus::Graveyard,
                    _ => panic!("expected approval status enum, got {}", e),
                };
                return Ok(mode);
            }
        }
        panic!("did not match any enum");
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 7,
            _ => false,
        }
    }
}

impl ToSql for ApprovalStatus {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> TResult<IsNull> {
        let status = match self {
            ApprovalStatus::Loved => "loved",
            ApprovalStatus::Qualified => "qualified",
            ApprovalStatus::Approved => "approved",
            ApprovalStatus::Ranked => "ranked",
            ApprovalStatus::Pending => "pending",
            ApprovalStatus::WIP => "wip",
            ApprovalStatus::Graveyard => "graveyard",
        };
        status.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 7,
            _ => false,
        }
    }
}

impl<'a> FromSql<'a> for Language {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> TResult<Self> {
        let enums = match *ty.kind() {
            Kind::Enum(ref enums) => enums,
            _ => panic!("expected enum type"),
        };
        for e in enums.iter() {
            if e.as_bytes() == raw {
                let language = match e {
                    "any" => Language::Any,
                    "other" => Language::Other,
                    "english" => Language::English,
                    "japanese" => Language::Japanese,
                    "chinese" => Language::Chinese,
                    "instrumental" => Language::Instrumental,
                    "korean" => Language::Korean,
                    "french" => Language::French,
                    "german" => Language::German,
                    "swedish" => Language::Swedish,
                    "spanish" => Language::Spanish,
                    "italian" => Language::Italian,
                    "russian" => Language::Russian,
                    "polish" => Language::Polish,
                    "unspecified" => Language::Unspecified,
                    _ => panic!("expected language enum, got {}", e),
                };
                return Ok(language);
            }
        }
        panic!("did not match any enum");
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 15,
            _ => false,
        }
    }
}

impl ToSql for Language {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> TResult<IsNull> {
        let language = match self {
            Language::Any => "any",
            Language::Other => "other",
            Language::English => "english",
            Language::Japanese => "japanese",
            Language::Chinese => "chinese",
            Language::Instrumental => "instrumental",
            Language::Korean => "korean",
            Language::French => "french",
            Language::German => "german",
            Language::Swedish => "swedish",
            Language::Spanish => "spanish",
            Language::Italian => "italian",
            Language::Russian => "russian",
            Language::Polish => "polish",
            Language::Unspecified => "unspecified",
        };
        language.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 15,
            _ => false,
        }
    }
}

impl<'a> FromSql<'a> for Genre {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> TResult<Self> {
        let enums = match *ty.kind() {
            Kind::Enum(ref enums) => enums,
            _ => panic!("expected enum type"),
        };
        for e in enums.iter() {
            if e.as_bytes() == raw {
                let genre = match e {
                    "unspecified" => Genre::Unspecified,
                    "videogame" => Genre::VideoGame,
                    "anime" => Genre::Anime,
                    "rock" => Genre::Rock,
                    "pop" => Genre::Pop,
                    "other" => Genre::Other,
                    "novelty" => Genre::Novelty,
                    "hiphop" => Genre::HipHop,
                    "electronic" => Genre::Electronic,
                    "metal" => Genre::Metal,
                    "classical" => Genre::Classical,
                    "folk" => Genre::Folk,
                    "jazz" => Genre::Jazz,
                    "any" => Genre::Any,
                    _ => panic!("expected genre enum, got {}", e),
                };
                return Ok(genre);
            }
        }
        panic!("did not match any enum");
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 14,
            _ => false,
        }
    }
}

impl ToSql for Genre {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> TResult<IsNull> {
        let genre = match self {
            Genre::Unspecified => "unspecified",
            Genre::VideoGame => "videogame",
            Genre::Anime => "anime",
            Genre::Rock => "rock",
            Genre::Pop => "pop",
            Genre::Other => "other",
            Genre::Novelty => "novelty",
            Genre::HipHop => "hiphop",
            Genre::Electronic => "electronic",
            Genre::Metal => "metal",
            Genre::Classical => "classical",
            Genre::Folk => "folk",
            Genre::Jazz => "jazz",
            Genre::Any => "any",
        };
        genre.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 14,
            _ => false,
        }
    }
}
