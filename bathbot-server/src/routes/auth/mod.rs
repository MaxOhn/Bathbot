use serde::{Deserialize, Serialize, Serializer};

pub mod osu;
pub mod twitch;

#[derive(Deserialize)]
pub struct Params {
    state: u8,
    code: String,
}

#[derive(Serialize)]
struct RenderData<'n> {
    #[serde(rename(serialize = "body_id"))]
    status: RenderDataStatus,
    kind: RenderDataKind,
    name: &'n str,
}

enum RenderDataStatus {
    Success,
    Error,
}

impl Serialize for RenderDataStatus {
    #[inline]
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let text = match self {
            RenderDataStatus::Success => "success",
            RenderDataStatus::Error => "error",
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

pub async fn auth_css() -> Vec<u8> {
    todo!()
}

pub async fn auth_icon() -> Vec<u8> {
    todo!()
}
