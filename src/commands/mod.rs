mod fun;
pub mod help;
mod twitch;
mod utility;

use fun::*;
use twitch::*;
use utility::*;

use crate::core::CommandGroup;

pub fn command_groups() -> Vec<CommandGroup> {
    vec![
        CommandGroup::new("osu", vec![]),
        CommandGroup::new("taiko", vec![]),
        CommandGroup::new("catch the beat", vec![]),
        CommandGroup::new("mania", vec![]),
        CommandGroup::new("fun", vec![]),
        CommandGroup::new("twitch", vec![]),
        CommandGroup::new("utility", vec![&PING_CMD, &ABOUT_CMD]),
    ]
}
