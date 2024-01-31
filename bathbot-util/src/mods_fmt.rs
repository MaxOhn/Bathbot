use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_v2::model::mods::{
    DaycoreCatch, DaycoreMania, DaycoreOsu, DaycoreTaiko, DoubleTimeCatch, DoubleTimeMania,
    DoubleTimeOsu, DoubleTimeTaiko, GameMod, GameMods, HalfTimeCatch, HalfTimeMania, HalfTimeOsu,
    HalfTimeTaiko, NightcoreCatch, NightcoreMania, NightcoreOsu, NightcoreTaiko,
};

pub struct ModsFormatter<'a> {
    mods: &'a GameMods,
}

impl<'a> ModsFormatter<'a> {
    pub fn new(mods: &'a GameMods) -> Self {
        Self { mods }
    }
}

impl Display for ModsFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.mods.is_empty() {
            return f.write_str("NM");
        }

        for gamemod in self.mods.iter() {
            f.write_str(gamemod.acronym().as_str())?;

            match gamemod {
                GameMod::HalfTimeOsu(HalfTimeOsu { speed_change, .. })
                | GameMod::DaycoreOsu(DaycoreOsu { speed_change, .. })
                | GameMod::DoubleTimeOsu(DoubleTimeOsu { speed_change, .. })
                | GameMod::NightcoreOsu(NightcoreOsu { speed_change, .. })
                | GameMod::HalfTimeTaiko(HalfTimeTaiko { speed_change, .. })
                | GameMod::DaycoreTaiko(DaycoreTaiko { speed_change, .. })
                | GameMod::DoubleTimeTaiko(DoubleTimeTaiko { speed_change, .. })
                | GameMod::NightcoreTaiko(NightcoreTaiko { speed_change, .. })
                | GameMod::HalfTimeCatch(HalfTimeCatch { speed_change, .. })
                | GameMod::DaycoreCatch(DaycoreCatch { speed_change, .. })
                | GameMod::DoubleTimeCatch(DoubleTimeCatch { speed_change, .. })
                | GameMod::NightcoreCatch(NightcoreCatch { speed_change, .. })
                | GameMod::HalfTimeMania(HalfTimeMania { speed_change, .. })
                | GameMod::DaycoreMania(DaycoreMania { speed_change, .. })
                | GameMod::DoubleTimeMania(DoubleTimeMania { speed_change, .. })
                | GameMod::NightcoreMania(NightcoreMania { speed_change, .. }) => {
                    if let Some(speed_change) = speed_change {
                        write!(f, "({}x)", (*speed_change * 100.0).round() / 100.0)?
                    }
                }

                GameMod::DifficultyAdjustOsu(_)
                | GameMod::DifficultyAdjustTaiko(_)
                | GameMod::DifficultyAdjustCatch(_)
                | GameMod::DifficultyAdjustMania(_) => {} // TODO: print something?

                _ => {}
            }
        }

        Ok(())
    }
}
