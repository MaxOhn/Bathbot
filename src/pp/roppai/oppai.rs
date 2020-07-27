#![allow(non_camel_case_types)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use super::OppaiErr;

use rosu::models::GameMods;
use std::ffi::{CStr, CString};

/// Wrapper struct for [oppai-ng](https://github.com/Francesco149/oppai-ng)'s `ezpp` struct in C code.
///
/// Notes from oppai-ng (some are omitted as the wrapper doesn't include the function):
///
/// - if map is "-" the map is read from standard input
/// - setting mods resets aim_stars and speed_stars, set those last
/// - setting end resets accuracy_percent
/// - mode defaults to MODE_STD or the map's mode
/// - mods default to MODS_NOMOD
/// - combo defaults to full combo
/// - nmiss defaults to 0
/// - if accuracy_percent is set, n300/100/50 are automatically
///   calculated and stored
/// - if n300/100/50 are set, accuracy_percent is automatically
///   calculated and stored
/// - if none of the above are set, SS (100%) is assumed
/// - if end is set, the map will be cut to this object index
pub struct Oppai {
    ezpp: ezpp_t,
}

impl Default for Oppai {
    fn default() -> Self {
        Self {
            ezpp: unsafe { ezpp_new() },
        }
    }
}

impl Oppai {
    // Setup ----------------------------------------------------------------------

    pub fn new() -> Self {
        Self::default()
    }

    /// oppai-ng: mode defaults to MODE_STD or the map's mode
    pub fn set_mode(&mut self, mode: u8) -> &mut Self {
        unsafe { ezpp_set_mode(self.ezpp, mode as i32) };
        self
    }

    /// oppai-ng: setting mods resets aim_stars and speed_stars, set those last
    ///
    /// mods default to MODS_NOMOD
    pub fn set_mods(&mut self, mods: u32) -> &mut Self {
        unsafe { ezpp_set_mods(self.ezpp, mods as i32) };
        self
    }

    /// oppai-ng: combo defaults to full combo
    pub fn set_combo(&mut self, combo: u32) -> &mut Self {
        unsafe { ezpp_set_combo(self.ezpp, combo as i32) };
        self
    }

    /// oppai-ng: miss count defaults to 0
    pub fn set_miss_count(&mut self, amount: u32) -> &mut Self {
        unsafe { ezpp_set_nmiss(self.ezpp, amount as i32) };
        self
    }

    /// Between 0.0 and 100.0
    ///
    /// oppai-ng: if accuracy_percent is set, n300/100/50 are automatically
    /// calculated and stored
    pub fn set_accuracy(&mut self, percent: f32) -> &mut Self {
        unsafe { ezpp_set_accuracy_percent(self.ezpp, percent) };
        self
    }

    /// oppai-ng: if n300/100/50 are set, accuracy_percent is automatically
    /// calculated and stored
    pub fn set_hits(&mut self, count100: u32, count50: u32) -> &mut Self {
        unsafe { ezpp_set_accuracy(self.ezpp, count100 as i32, count50 as i32) };
        self
    }

    /// oppai-ng: setting end resets accuracy_percent
    ///
    /// if end is set, the map will be cut to this object index
    pub fn set_end_index(&mut self, index: u32) -> &mut Self {
        unsafe { ezpp_set_end(self.ezpp, index as i32) };
        self
    }

    /// If None is provided, it will reuse the previously used path if there is one,
    /// otherwise it returns an Err
    ///
    /// oppai-ng: if map is "-" the map is read from standard input
    pub fn calculate(&mut self, map_path: &str) -> Result<&mut Self, OppaiErr> {
        let file_content = CString::new(map_path).map_err(|why| {
            OppaiErr::Format(format!(
                "Could not translate {} to CString: {}",
                map_path, why
            ))
        })?;
        match unsafe { ezpp(self.ezpp, file_content.as_ptr() as *mut _) } {
            code if code < 0 => {
                let raw = unsafe { errstr(code) };
                let msg = unsafe { CStr::from_ptr(raw) }.to_str().map_err(|why| {
                    OppaiErr::Binding(format!(
                        "Error while transforming CString error msg into String: {}",
                        why
                    ))
                })?;
                Err(OppaiErr::new(code, msg))
            }
            _ => Ok(self),
        }
    }

    // ----------------------------------------------------------------------------

    // Usage ----------------------------------------------------------------------

    pub fn get_pp(&self) -> f32 {
        unsafe { ezpp_pp(self.ezpp) }
    }

    pub fn get_stars(&self) -> f32 {
        unsafe { ezpp_stars(self.ezpp) }
    }

    pub fn get_accuracy(&self) -> f32 {
        unsafe { ezpp_accuracy_percent(self.ezpp) }
    }

    pub fn get_count300(&self) -> u32 {
        unsafe { ezpp_n300(self.ezpp) as u32 }
    }

    pub fn get_count100(&self) -> u32 {
        unsafe { ezpp_n100(self.ezpp) as u32 }
    }

    pub fn get_count50(&self) -> u32 {
        unsafe { ezpp_n50(self.ezpp) as u32 }
    }

    pub fn get_miss_count(&self) -> u32 {
        unsafe { ezpp_nmiss(self.ezpp) as u32 }
    }

    pub fn get_ar(&self) -> f32 {
        unsafe { ezpp_ar(self.ezpp) }
    }

    pub fn get_cs(&self) -> f32 {
        unsafe { ezpp_cs(self.ezpp) }
    }

    pub fn get_od(&self) -> f32 {
        unsafe { ezpp_od(self.ezpp) }
    }

    pub fn get_hp(&self) -> f32 {
        unsafe { ezpp_hp(self.ezpp) }
    }

    pub fn get_object_count(&self) -> usize {
        unsafe { ezpp_nobjects(self.ezpp) as usize }
    }

    pub fn get_combo(&self) -> u32 {
        unsafe { ezpp_combo(self.ezpp) as u32 }
    }

    pub fn get_max_combo(&self) -> u32 {
        unsafe { ezpp_max_combo(self.ezpp) as u32 }
    }

    pub fn get_mods(&self) -> GameMods {
        unsafe { GameMods::from_bits(ezpp_mods(self.ezpp) as u32).unwrap() }
    }

    pub fn get_time_at(&self, idx: usize) -> u32 {
        unsafe { ezpp_time_at(self.ezpp, idx as i32) as u32 }
    }

    pub fn get_strain_at(&self, idx: usize, difficulty_type: i32) -> f32 {
        unsafe { ezpp_strain_at(self.ezpp, idx as i32, difficulty_type) }
    }

    // ----------------------------------------------------------------------------
}

impl Drop for Oppai {
    fn drop(&mut self) {
        unsafe { ezpp_free(self.ezpp) }
    }
}
