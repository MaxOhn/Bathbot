use rosu::models::{ApprovalStatus, GameMode, Genre, Language};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "GameMode")]
#[repr(u8)]
pub enum GameModeDef {
    STD = 0,
    TKO = 1,
    CTB = 2,
    MNA = 3,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Genre")]
pub enum GenreDef {}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Language")]
pub enum LanguageDef {}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ApprovalStatus")]
pub enum ApprovalStatusDef {}
