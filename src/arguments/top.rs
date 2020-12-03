use super::{parse_dotted, Args};
use crate::{
    commands::osu::TopSortBy,
    util::{matcher, osu::ModSelection},
    Context,
};

use rosu::model::Grade;

pub struct TopArgs {
    pub name: Option<String>,
    pub mods: Option<ModSelection>,
    pub acc_min: Option<f32>,
    pub acc_max: Option<f32>,
    pub combo_min: Option<u32>,
    pub combo_max: Option<u32>,
    pub grade: Option<Grade>,
    pub sort_by: TopSortBy,
    pub has_dash_r: bool,
    pub has_dash_p: bool,
}

impl TopArgs {
    pub fn new(ctx: &Context, args: Args) -> Result<Self, &'static str> {
        let mut args: Vec<_> = args.take(8).map(str::to_owned).collect();

        let mut acc_min = None;
        let mut acc_max = None;
        if let Some(idx) = args.iter().position(|arg| arg == "-a") {
            args.remove(idx);
            if let Some((min, minmax)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                if let Some(min) = min {
                    acc_min.replace(min);
                    acc_max.replace(minmax);
                } else {
                    acc_min.replace(minmax);
                }
            } else {
                return Err("After the acc keyword you must specify either \
                    a decimal number for min acc or two decimal numbers \
                    of the form `a..b` for min and max acc");
            }
        }

        let mut combo_min = None;
        let mut combo_max = None;
        if let Some(idx) = args.iter().position(|arg| arg == "-c") {
            args.remove(idx);
            if let Some((min, minmax)) = args.get(idx).and_then(parse_dotted) {
                args.remove(idx);
                if let Some(min) = min {
                    combo_min.replace(min);
                    combo_max.replace(minmax);
                } else {
                    combo_min.replace(minmax);
                }
            } else {
                return Err("After the combo keyword you must specify either \
                            an integer for min combo or two integer numbers of the \
                            form `a..b` for min and max combo");
            }
        }
        let grade = super::grade(&mut args)?;
        let mods = super::mods(&mut args);
        let sort_by = if super::keywords(&mut args, &["--a", "--acc"]) {
            TopSortBy::Acc
        } else if super::keywords(&mut args, &["--c", "--combo"]) {
            TopSortBy::Combo
        } else {
            TopSortBy::None
        };
        let has_dash_r = super::keywords(&mut args, &["-r"]);
        let has_dash_p = super::keywords(&mut args, &["-p"]);
        let name = args.pop().and_then(|arg| {
            matcher::get_mention_user(&arg)
                .and_then(|id| ctx.get_link(id))
                .or(Some(arg))
        });
        Ok(Self {
            name,
            mods,
            acc_min,
            acc_max,
            combo_min,
            combo_max,
            grade,
            sort_by,
            has_dash_r,
            has_dash_p,
        })
    }
}
