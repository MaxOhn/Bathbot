use rkyv::{niche::niching::Null, with::NicheInto};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
pub struct ScrapedUser {
    #[serde(rename = "achievements")]
    pub medals: Box<[ScrapedMedal]>,
}

#[derive(Debug, Deserialize, rkyv::Archive, rkyv::Serialize)]
pub struct ScrapedMedal {
    pub icon_url: Box<str>,
    pub id: u16,
    pub name: Box<str>,
    pub grouping: Box<str>,
    pub ordering: u8,
    pub description: Box<str>,
    #[serde(default, deserialize_with = "deser_mode")]
    #[rkyv(with = NicheInto<Null>)]
    pub mode: Option<Box<str>>,
    #[rkyv(with = NicheInto<Null>)]
    pub instructions: Option<Box<str>>,
}

fn deser_mode<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Box<str>>, D::Error> {
    match Option::<&str>::deserialize(d) {
        Ok(Some("fruits")) => Ok(Some(Box::from("catch"))),
        Ok(Some(mode)) => Ok(Some(Box::from(mode))),
        Ok(None) => Ok(None),
        Err(err) => Err(err),
    }
}
