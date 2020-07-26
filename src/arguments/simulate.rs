use super::{ArgResult, Args};
use crate::util::{matcher, osu::ModSelection};

use rosu::models::GameMods;
use std::iter::FromIterator;

pub struct SimulateArgs {
    pub mods: Option<ModSelection>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

impl SimulateArgs {
    pub fn is_some(&self) -> bool {
        self.acc.is_some()
            || self.mods.is_some()
            || self.combo.is_some()
            || self.miss.is_some()
            || self.score.is_some()
            || self.n300.is_some()
            || self.n100.is_some()
            || self.n50.is_some()
    }
}

impl Into<SimulateArgs> for SimulateMapArgs {
    fn into(self) -> SimulateArgs {
        SimulateArgs {
            mods: self.mods,
            score: self.score,
            n300: self.n300,
            n100: self.n100,
            n50: self.n50,
            miss: self.miss,
            acc: self.acc,
            combo: self.combo,
        }
    }
}

impl Into<SimulateArgs> for SimulateNameArgs {
    fn into(self) -> SimulateArgs {
        SimulateArgs {
            mods: self.mods,
            score: self.score,
            n300: self.n300,
            n100: self.n100,
            n50: self.n50,
            miss: self.miss,
            acc: self.acc,
            combo: self.combo,
        }
    }
}

pub struct SimulateMapArgs {
    pub map_id: Option<u32>,
    pub mods: Option<ModSelection>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

pub struct SimulateNameArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

impl SimulateMapArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take(16).map(|arg| arg.to_owned()).collect();
        let mods = super::mods(&mut args);
        let acc = super::acc(&mut args)?;
        let combo = super::combo(&mut args)?;
        let miss = super::miss(&mut args)?;
        let n300 = super::n300(&mut args)?;
        let n100 = super::n100(&mut args)?;
        let n50 = super::n50(&mut args)?;
        let score = super::score(&mut args)?;
        let map_id = args.pop().and_then(|arg| matcher::get_osu_map_id(&arg));
        Ok(Self {
            map_id,
            mods,
            acc,
            combo,
            score,
            miss,
            n300,
            n100,
            n50,
        })
    }
}

impl SimulateNameArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take(16).map(|arg| arg.to_owned()).collect();
        let mods = super::mods(&mut args);
        let acc = super::acc(&mut args)?;
        let combo = super::combo(&mut args)?;
        let miss = super::miss(&mut args)?;
        let n300 = super::n300(&mut args)?;
        let n100 = super::n100(&mut args)?;
        let n50 = super::n50(&mut args)?;
        let score = super::score(&mut args)?;
        let name = args.pop();
        Ok(Self {
            name,
            mods,
            acc,
            combo,
            score,
            miss,
            n300,
            n100,
            n50,
        })
    }
}
