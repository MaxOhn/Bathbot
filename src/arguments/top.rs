use super::Args;
use crate::{
    commands::osu::TopSortBy,
    util::{matcher, osu::ModSelection},
    Context,
};

use rosu::models::Grade;

pub struct TopArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
    pub acc: Option<f32>,
    pub combo: Option<u32>,
    pub grade: Option<Grade>,
    pub sort_by: TopSortBy,
}

impl TopArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
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
        let name = args.pop().and_then(|arg| {
            matcher::get_mention_user(&arg)
                .and_then(|id| ctx.get_link(id))
                .or_else(|| Some(arg))
        });
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
