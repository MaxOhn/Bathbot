pub mod fun;
pub mod help;
pub mod osu;
pub mod owner;
pub mod songs;
pub mod twitch;
pub mod utility;

use fun::*;
use osu::*;
use owner::*;
use songs::*;
use twitch::*;
use utility::*;

use crate::core::CommandGroup;

pub fn command_groups() -> Vec<CommandGroup> {
    vec![
        CommandGroup::new(
            "all osu! modes",
            vec![
                &LINK_CMD,
                &SCORES_CMD,
                &SIMULATE_CMD,
                &MAP_CMD,
                &MATCHCOSTS_CMD,
                &AVATAR_CMD,
                &MOSTPLAYED_CMD,
                &MOSTPLAYEDCOMMON_CMD,
                &LEADERBOARD_CMD,
                &GLOBALLEADERBOARD_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!standard",
            vec![
                &RECENT_CMD,
                &TOP_CMD,
                &RECENTBEST_CMD,
                &PROFILE_CMD,
                &PP_CMD,
                &WHATIF_CMD,
                &RANK_CMD,
                &COMMON_CMD,
                &RECENTLEADERBOARD_CMD,
                &RECENTGLOBALLEADERBOARD_CMD,
                &OSUSTATSGLOBALS_CMD,
                &OSUSTATSCOUNT_CMD,
                &SIMULATERECENT_CMD,
                &NOCHOKES_CMD,
                &SOTARKS_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!mania",
            vec![
                &RECENTMANIA_CMD,
                &TOPMANIA_CMD,
                &RECENTBESTMANIA_CMD,
                &PROFILEMANIA_CMD,
                &PPMANIA_CMD,
                &WHATIFMANIA_CMD,
                &RANKMANIA_CMD,
                &COMMONMANIA_CMD,
                &RECENTMANIALEADERBOARD_CMD,
                &RECENTMANIAGLOBALLEADERBOARD_CMD,
                &OSUSTATSGLOBALSMANIA_CMD,
                &OSUSTATSCOUNTMANIA_CMD,
                &RATIOS_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!taiko",
            vec![
                &RECENTTAIKO_CMD,
                &TOPTAIKO_CMD,
                &RECENTBESTTAIKO_CMD,
                &PROFILETAIKO_CMD,
                &PPTAIKO_CMD,
                &WHATIFTAIKO_CMD,
                &RANKTAIKO_CMD,
                &COMMONTAIKO_CMD,
                &RECENTTAIKOLEADERBOARD_CMD,
                &RECENTTAIKOGLOBALLEADERBOARD_CMD,
                &OSUSTATSGLOBALSTAIKO_CMD,
                &OSUSTATSCOUNTTAIKO_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!catch the beat",
            vec![
                &RECENTCTB_CMD,
                &TOPCTB_CMD,
                &RECENTBESTCTB_CMD,
                &PROFILECTB_CMD,
                &PPCTB_CMD,
                &WHATIFCTB_CMD,
                &RANKCTB_CMD,
                &COMMONCTB_CMD,
                &RECENTCTBLEADERBOARD_CMD,
                &RECENTCTBGLOBALLEADERBOARD_CMD,
                &OSUSTATSGLOBALSCTB_CMD,
                &OSUSTATSCOUNTCTB_CMD,
            ],
        ),
        CommandGroup::new(
            "twitch",
            vec![&ADDSTREAM_CMD, &REMOVESTREAM_CMD, &TRACKEDSTREAMS_CMD],
        ),
        CommandGroup::new(
            "fun",
            vec![
                &MINESWEEPER_CMD,
                &BACKGROUNDGAME_CMD,
                &BGTAGS_CMD,
                &BGTAGSMANUAL_CMD,
            ],
        ),
        CommandGroup::new(
            "utility",
            vec![
                &PING_CMD,
                &ABOUT_CMD,
                &COMMANDS_CMD,
                &INVITE_CMD,
                &PRUNE_CMD,
                &PREFIX_CMD,
                &ECHO_CMD,
                &AUTHORITIES_CMD,
                &ROLEASSIGN_CMD,
                &LYRICS_CMD,
            ],
        ),
        CommandGroup::new(
            "songs",
            vec![
                &BOMBSAWAY_CMD,
                &CATCHIT_CMD,
                &DING_CMD,
                &FIREANDFLAMES_CMD,
                &FIREFLIES_CMD,
                &FLAMINGO_CMD,
                &PRETENDER_CMD,
                &ROCKEFELLER_CMD,
                &SAYGOODBYE_CMD,
                &TIJDMACHINE_CMD,
            ],
        ),
        CommandGroup::new("owner", vec![&ADDBG_CMD]),
    ]
}
