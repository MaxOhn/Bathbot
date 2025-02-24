use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};

use self::error::AuthError;

pub mod error;
pub mod osu;
pub mod twitch;

#[derive(Deserialize)]
pub struct Params {
    state: u8,
    code: String,
}

struct RenderData<'n> {
    status: RenderDataStatus<'n>,
    kind: RenderDataKind,
}

impl Serialize for RenderData<'_> {
    #[inline]
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;

        map.serialize_entry("body_id", &self.status)?;
        map.serialize_entry("kind", &self.kind)?;

        match self.status {
            RenderDataStatus::Success { name } => map.serialize_entry("name", name)?,
            RenderDataStatus::Error { msg } => map.serialize_entry("error", msg)?,
        }

        map.end()
    }
}

#[derive(Copy, Clone)]
enum RenderDataStatus<'s> {
    Success { name: &'s str },
    Error { msg: &'s str },
}

impl Serialize for RenderDataStatus<'_> {
    #[inline]
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let text = match self {
            RenderDataStatus::Success { .. } => "success",
            RenderDataStatus::Error { .. } => "error",
        };

        s.serialize_str(text)
    }
}

enum RenderDataKind {
    Osu,
    Twitch,
}

impl Serialize for RenderDataKind {
    #[inline]
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let text = match self {
            RenderDataKind::Osu => "osu!",
            RenderDataKind::Twitch => "twitch",
        };

        s.serialize_str(text)
    }
}
