use super::Args;
use crate::util::{matcher, osu::ModSelection};

use itertools::Itertools;
use std::str::FromStr;

pub struct NameArgs {
    pub name: Option<String>,
}

impl NameArgs {
    pub fn new(mut args: Args) -> Self {
        Self {
            name: args.single::<String>().ok(),
        }
    }
}

pub struct MultNameArgs {
    pub names: Vec<String>,
}

impl MultNameArgs {
    pub fn new(args: Args, n: usize) -> Self {
        let names = args.take(n).unique().map(|arg| arg.to_owned()).collect();
        Self { names }
    }
}

pub struct NameFloatArgs {
    pub name: Option<String>,
    pub float: f32,
}

impl NameFloatArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take_all();
        let float = match args.next_back().and_then(|arg| f32::from_str(&arg).ok()) {
            Some(float) => float,
            None => return Err("You need to provide a decimal number as last argument"),
        };
        Ok(Self {
            name: args.next().map(|arg| arg.to_owned()),
            float,
        })
    }
}

pub struct NameIntArgs {
    pub name: Option<String>,
    pub number: Option<u32>,
}

impl NameIntArgs {
    pub fn new(args: Args) -> Self {
        let mut name = None;
        let mut number = None;
        for arg in args {
            let res = u32::from_str(arg).ok();
            if res.is_some() {
                number = res;
            } else {
                name = Some(arg.to_owned());
            }
        }
        Self { name, number }
    }
}

pub struct NameModArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
}

impl NameModArgs {
    pub fn new(args: Args) -> Self {
        let mut name = None;
        let mut mods = None;
        for arg in args {
            let res = matcher::get_mods(arg);
            if res.is_some() {
                mods = res;
            } else {
                name = Some(arg.to_owned());
            }
        }
        Self { name, mods }
    }
}
