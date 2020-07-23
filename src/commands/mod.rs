pub mod fun;
pub mod help;
pub mod twitch;
pub mod utility;

use fun::*;
use twitch::*;
use utility::*;

use crate::core::CommandGroup;

fn command_issue(cmd: &str) -> String {
    format!("Some issue while preparing `{}` response, blame bade", cmd)
}

pub fn command_groups() -> Vec<CommandGroup> {
    vec![
        // TODO: Re-enable when used
        // CommandGroup::new("osu", vec![]),
        // CommandGroup::new("taiko", vec![]),
        // CommandGroup::new("catch the beat", vec![]),
        // CommandGroup::new("mania", vec![]),
        // CommandGroup::new("fun", vec![]),
        CommandGroup::new(
            "twitch",
            vec![&ADDSTREAM_CMD, &REMOVESTREAM_CMD, &TRACKEDSTREAMS_CMD],
        ),
        CommandGroup::new(
            "utility",
            vec![
                &PING_CMD,
                &ABOUT_CMD,
                &COMMANDS_CMD,
                &AVATAR_CMD,
                &INVITE_CMD,
                &PRUNE_CMD,
                &PREFIX_CMD,
                &ECHO_CMD,
                &AUTHORITIES_CMD,
                &ROLEASSIGN_CMD,
                &LYRICS_CMD,
            ],
        ),
    ]
}
