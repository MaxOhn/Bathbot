use crate::arguments::{self, ModSelection};

use rosu::models::GameMods;
use serenity::framework::standard::Args;
use std::str::FromStr;

pub struct NameArgs {
    pub name: Option<String>,
}

impl NameArgs {
    pub fn new(mut args: Args) -> Self {
        let mut args = arguments::first_n(&mut args, 1);
        Self { name: args.pop() }
    }
}

pub struct NamePassArgs {
    pub name: Option<String>,
    pub pass: bool,
}

impl NamePassArgs {
    pub fn new(mut args: Args) -> Self {
        let args = arguments::first_n(&mut args, 2);
        let mut name = None;
        let mut pass = false;
        for arg in args {
            if arg.as_str() == "-pass" || arg.as_str() == "-passes" {
                pass = true;
            } else {
                name = Some(arg)
            }
        }
        Self { name, pass }
    }
}

pub struct MultNameArgs {
    pub names: Vec<String>,
}

impl MultNameArgs {
    pub fn new(mut args: Args, n: usize) -> Self {
        let args = arguments::first_n(&mut args, n);
        Self { names: args }
    }
}

pub struct NameFloatArgs {
    pub name: Option<String>,
    pub float: f32,
}

impl NameFloatArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 2);
        let float = args.pop().and_then(|arg| f32::from_str(&arg).ok());
        if float.is_none() {
            return Err("You need to provide a decimal \
                        number as last argument"
                .to_string());
        }
        Ok(Self {
            name: args.pop(),
            float: float.unwrap(),
        })
    }
}

pub struct NameModArgs {
    pub name: Option<String>,
    pub mods: Option<(GameMods, ModSelection)>,
}

impl NameModArgs {
    pub fn new(mut args: Args) -> Self {
        let args = arguments::first_n(&mut args, 2);
        let mut name = None;
        let mut mods = None;
        for arg in args {
            let res = arguments::parse_mods(&arg);
            if res.is_some() {
                mods = res;
            } else {
                name = Some(arg);
            }
        }
        Self { name, mods }
    }
}
