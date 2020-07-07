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
                    "osu" => Language::STD,
                    "taiko" => Language::TKO,
                    "fruits" => Language::CTB,
                    "mania" => Language::MNA,
                    "" => Language::_,
                    "" => Language::_,
                    "" => Language::_,
                    "" => Language::_,
                    "" => Language::_,
                    "" => Language::_,
                    "" => Language::_,
                    "" => Language::_,
                    _ => panic!("expected language enum, got {}", e),
                };
                return Ok(language);
            }
        }
        panic!("did not match any enum");
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 4, // TODO
            _ => false,
        }
    }
}

impl ToSql for Language {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> TResult<IsNull> {
        let language = match self {
            Language::STD => "osu",
            Language::TKO => "taiko",
            Language::CTB => "fruits",
            Language::MNA => "mania",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
            Language::_ => "",
        };
        language.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 4, // TODO
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
                    "osu" => Genre::STD,
                    "taiko" => Genre::TKO,
                    "fruits" => Genre::CTB,
                    "mania" => Genre::MNA,
                    "" => Genre::_,
                    "" => Genre::_,
                    "" => Genre::_,
                    "" => Genre::_,
                    "" => Genre::_,
                    "" => Genre::_,
                    _ => panic!("expected genre enum, got {}", e),
                };
                return Ok(genre);
            }
        }
        panic!("did not match any enum");
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 4, // TODO
            _ => false,
        }
    }
}

impl ToSql for Genre {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> TResult<IsNull> {
        let genre = match self {
            Genre::STD => "osu",
            Genre::TKO => "taiko",
            Genre::CTB => "fruits",
            Genre::MNA => "mania",
            Genre::_ => "",
            Genre::_ => "",
            Genre::_ => "",
            Genre::_ => "",
        };
        genre.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        match *ty.kind() {
            Kind::Enum(ref enums) => enums.len() == 4, // TODO
            _ => false,
        }
    }
}

