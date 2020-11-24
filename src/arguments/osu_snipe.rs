use super::Args;
use crate::{custom_client::SnipeScoreOrder, util::osu::ModSelection};

pub struct SnipeScoreArgs {
    pub name: Option<String>,
    pub order: SnipeScoreOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl SnipeScoreArgs {
    pub fn new(args: Args) -> Self {
        let mut args: Vec<_> = args.take(4).map(str::to_owned).collect();
        // Parse mods
        let mods = super::mods(&mut args);
        // Parse descending/ascending
        let descending = !super::keywords(&mut args, &["--asc", "--ascending"]);
        // Parse order
        let order = if super::keywords(&mut args, &["--a", "--acc"]) {
            SnipeScoreOrder::Accuracy
        } else if super::keywords(&mut args, &["--md", "--mapdate"]) {
            SnipeScoreOrder::MapApprovalDate
        } else if super::keywords(&mut args, &["--m", "--miss", "--misses"]) {
            SnipeScoreOrder::Misses
        } else if super::keywords(&mut args, &["--sd", "--scoredate"]) {
            SnipeScoreOrder::ScoreDate
        } else if super::keywords(&mut args, &["--s", "--stars"]) {
            SnipeScoreOrder::Stars
        } else if super::keywords(&mut args, &["--l", "--len", "--length"]) {
            SnipeScoreOrder::Length
        } else {
            SnipeScoreOrder::Pp
        };
        Self {
            name: args.pop(),
            order,
            mods,
            descending,
        }
    }
}
