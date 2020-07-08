use super::DBMap;

pub struct CtbPP {
    pub beatmap_id: u32,
    pub NM: Option<f32>,
    pub HD: Option<f32>,
    pub HR: Option<f32>,
    pub DT: Option<f32>,
    pub HDHR: Option<f32>,
    pub HDDT: Option<f32>,
}

impl CtbPP {
    pub fn get(&self, bits: u32) -> Result<Option<f32>, Error> {
        let pp = match bits {
            0 => self.NM,
            8 => self.HD,
            16 => self.HR,
            24 => self.HDHR,
            64 => self.DT,
            72 => self.HDDT,
            _ => {
                bail!("{} are no valid mod bits for the ctb pp table", bits);
            }
        };
        Ok(pp)
    }
}
