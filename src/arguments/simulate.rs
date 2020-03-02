use crate::arguments::{self, ModSelection};

use rosu::models::GameMods;
use serenity::framework::standard::Args;

pub struct SimulateArgs {
    pub mods: Option<(GameMods, ModSelection)>,
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
    pub mods: Option<(GameMods, ModSelection)>,
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
    pub mods: Option<(GameMods, ModSelection)>,
    pub score: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub miss: Option<u32>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
}

impl SimulateMapArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 16);
        let mods = arguments::mods(&mut args);
        let acc = arguments::acc(&mut args)?;
        let combo = arguments::combo(&mut args)?;
        let miss = arguments::miss(&mut args)?;
        let n300 = arguments::n300(&mut args)?;
        let n100 = arguments::n100(&mut args)?;
        let n50 = arguments::n50(&mut args)?;
        let score = arguments::score(&mut args)?;
        let map_id = args.pop().and_then(|arg| arguments::get_regex_id(&arg));
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
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 16);
        let mods = arguments::mods(&mut args);
        let acc = arguments::acc(&mut args)?;
        let combo = arguments::combo(&mut args)?;
        let miss = arguments::miss(&mut args)?;
        let n300 = arguments::n300(&mut args)?;
        let n100 = arguments::n100(&mut args)?;
        let n50 = arguments::n50(&mut args)?;
        let score = arguments::score(&mut args)?;
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
