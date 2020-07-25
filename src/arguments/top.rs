use super::{ArgResult, Args};
use crate::util::osu::ModSelection;
// use crate::comands::osu::top::TopSortBy;

use rosu::models::{GameMods, Grade};
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
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take(8).map(|arg| arg.to_owned()).collect();
        let acc = super::acc(&mut args)?;
        let combo = super::combo(&mut args)?;
        let grade = super::grade(&mut args)?;
        let mods = super::mods(&mut args);
        let sort_by = if super::keywords(&mut args, &["--a", "--acc"]) {
            TopSortBy::Acc
        } else if super::keywords(&mut args, &["--c", "--combo"]) {
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
