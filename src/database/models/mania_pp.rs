#![allow(non_snake_case)]
use super::{super::schema::pp_mania_mods, beatmap::DBMap};

use failure::Error;

#[derive(Default, Copy, Clone, Identifiable, Queryable, Associations, Insertable, AsChangeset)]
#[table_name = "pp_mania_mods"]
#[belongs_to(DBMap, foreign_key = "beatmap_id")]
#[primary_key(beatmap_id)]
pub struct ManiaPP {
    pub beatmap_id: u32,
    pub NM: Option<f32>,
    pub NF: Option<f32>,
    pub EZ: Option<f32>,
    pub DT: Option<f32>,
    pub HT: Option<f32>,
    pub NFEZ: Option<f32>,
    pub NFDT: Option<f32>,
    pub EZDT: Option<f32>,
    pub NFHT: Option<f32>,
    pub EZHT: Option<f32>,
    pub NFEZDT: Option<f32>,
    pub NFEZHT: Option<f32>,
}

impl ManiaPP {
    pub fn get(&self, bits: u32) -> Result<Option<f32>, Error> {
        let pp = match bits {
            0 => self.NM,
            1 => self.NF,
            2 => self.EZ,
            3 => self.NFEZ,
            64 => self.DT,
            65 => self.NFDT,
            66 => self.EZDT,
            67 => self.NFEZDT,
            256 => self.HT,
            257 => self.NFHT,
            258 => self.EZHT,
            259 => self.NFEZHT,
            _ => {
                bail!("{} are no valid mod bits for the mania pp table", bits);
            }
        };
        Ok(pp)
    }

    pub fn new(map_id: u32, bits: u32, value: Option<f32>) -> Result<Self, Error> {
        let mut pp = Self::default();
        pp.beatmap_id = map_id;
        match bits {
            0 => pp.NM = value,
            1 => pp.NF = value,
            2 => pp.EZ = value,
            3 => pp.NFEZ = value,
            64 => pp.DT = value,
            65 => pp.NFDT = value,
            66 => pp.EZDT = value,
            67 => pp.NFEZDT = value,
            256 => pp.HT = value,
            257 => pp.NFHT = value,
            258 => pp.EZHT = value,
            259 => pp.NFEZHT = value,
            _ => {
                bail!("{} are no valid mod bits for the mania pp table", bits);
            }
        };
        Ok(pp)
    }
}
