use std::fmt::{Formatter, Result as FmtResult};

use rosu_v2::prelude::{GameMode, GameMods, ModeAsSeed};
use serde::{
    de::{DeserializeSeed, Error as DeError, Visitor},
    Deserializer,
};

pub struct MaybeMods(GameMode);

impl<'de> Visitor<'de> for MaybeMods {
    type Value = Option<GameMods>;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("optional GameMods")
    }

    fn visit_unit<E: DeError>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E: DeError>(self) -> Result<Self::Value, E> {
        self.visit_unit()
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        ModeAsSeed::<GameMods>::new(self.0).deserialize(d).map(Some)
    }
}
