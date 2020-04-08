use crate::{
    arguments::{self, ModSelection},
    commands::osu::top::TopSortBy,
};

use rosu::models::{GameMods, Grade};
use serenity::framework::standard::Args;
use std::iter::FromIterator;

pub struct TopArgs {
    pub name: Option<String>,
    pub mods: Option<(GameMods, ModSelection)>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
    pub grade: Option<Grade>,
    pub sort_by: TopSortBy,
}

impl TopArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = Vec::from_iter(arguments::first_n(&mut args, 8));
        let acc = arguments::acc(&mut args)?;
        let combo = arguments::combo(&mut args)?;
        let grade = arguments::grade(&mut args)?;
        let mods = arguments::mods(&mut args);
        let sort_by = if arguments::keywords(&mut args, &["--a", "--acc"]) {
            TopSortBy::Acc
        } else if arguments::keywords(&mut args, &["--c", "--combo"]) {
            TopSortBy::Combo
        } else {
            TopSortBy::None
        };
        let name = args.pop();
        Ok(Self {
            name,
            mods,
            acc,
            combo,
            grade,
            sort_by,
        })
    }
}
