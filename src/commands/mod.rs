pub mod fun;
pub mod help;
pub mod osu;
pub mod owner;
pub mod songs;
pub mod tracking;
pub mod twitch;
pub mod utility;

use fun::*;
use osu::*;
use owner::*;
use songs::*;
use tracking::*;
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
                &BWS_CMD,
                &AVATAR_CMD,
                &MOSTPLAYED_CMD,
                &MOSTPLAYEDCOMMON_CMD,
                &LEADERBOARD_CMD,
                &BELGIANLEADERBOARD_CMD,
                &MEDAL_CMD,
                &MEDALSTATS_CMD,
                &MEDALSMISSING_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!standard",
            vec![
                &RECENT_CMD,
                &TOP_CMD,
                &RECENTBEST_CMD,
                &OSU_CMD,
                &OSUCOMPARE_CMD,
                &PP_CMD,
                &WHATIF_CMD,
                &RANK_CMD,
                &COMMON_CMD,
                &RECENTLEADERBOARD_CMD,
                &RECENTBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALS_CMD,
                &OSUSTATSCOUNT_CMD,
                &OSUSTATSLIST_CMD,
                &SIMULATERECENT_CMD,
                &NOCHOKES_CMD,
                &SOTARKS_CMD,
                &MAPPER_CMD,
                &TOPIF_CMD,
                &TOPOLD_CMD,
                &SNIPED_CMD,
                &PLAYERSNIPESTATS_CMD,
                &PLAYERSNIPELIST_CMD,
                &COUNTRYSNIPESTATS_CMD,
                &COUNTRYSNIPELIST_CMD,
                &RANKRANKEDSCORE_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!mania",
            vec![
                &RECENTMANIA_CMD,
                &TOPMANIA_CMD,
                &RECENTBESTMANIA_CMD,
                &MANIA_CMD,
                &OSUCOMPAREMANIA_CMD,
                &PPMANIA_CMD,
                &WHATIFMANIA_CMD,
                &RANKMANIA_CMD,
                &COMMONMANIA_CMD,
                &RECENTMANIALEADERBOARD_CMD,
                &RECENTMANIABELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALSMANIA_CMD,
                &OSUSTATSCOUNTMANIA_CMD,
                &OSUSTATSLISTMANIA_CMD,
                &SIMULATERECENTMANIA_CMD,
                &RATIOS_CMD,
                &MAPPERMANIA_CMD,
                &TOPOLDMANIA_CMD,
                &RANKRANKEDSCOREMANIA_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!taiko",
            vec![
                &RECENTTAIKO_CMD,
                &TOPTAIKO_CMD,
                &RECENTBESTTAIKO_CMD,
                &TAIKO_CMD,
                &OSUCOMPARETAIKO_CMD,
                &PPTAIKO_CMD,
                &WHATIFTAIKO_CMD,
                &RANKTAIKO_CMD,
                &COMMONTAIKO_CMD,
                &RECENTTAIKOLEADERBOARD_CMD,
                &RECENTTAIKOBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALSTAIKO_CMD,
                &OSUSTATSCOUNTTAIKO_CMD,
                &OSUSTATSLISTTAIKO_CMD,
                &SIMULATERECENTTAIKO_CMD,
                &NOCHOKESTAIKO_CMD,
                &MAPPERTAIKO_CMD,
                &TOPIFTAIKO_CMD,
                &TOPOLDTAIKO_CMD,
                &RANKRANKEDSCORETAIKO_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!catch the beat",
            vec![
                &RECENTCTB_CMD,
                &TOPCTB_CMD,
                &RECENTBESTCTB_CMD,
                &CTB_CMD,
                &OSUCOMPARECTB_CMD,
                &PPCTB_CMD,
                &WHATIFCTB_CMD,
                &RANKCTB_CMD,
                &COMMONCTB_CMD,
                &RECENTCTBLEADERBOARD_CMD,
                &RECENTCTBBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALSCTB_CMD,
                &OSUSTATSCOUNTCTB_CMD,
                &OSUSTATSLISTCTB_CMD,
                &SIMULATERECENTCTB_CMD,
                &NOCHOKESCTB_CMD,
                &MAPPERCTB_CMD,
                &TOPIFCTB_CMD,
                &TOPOLDCTB_CMD,
                &RANKRANKEDSCORECTB_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!tracking",
            vec![
                &TRACK_CMD,
                &TRACKMANIA_CMD,
                &TRACKTAIKO_CMD,
                &TRACKCTB_CMD,
                &TRACKLIST_CMD,
                &UNTRACK_CMD,
                &UNTRACKALL_CMD,
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
                &ROLL_CMD,
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
        CommandGroup::new(
            "owner",
            vec![
                &ADDBG_CMD,
                &CACHE_CMD,
                &ACTIVEBG_CMD,
                &CHANGEGAME_CMD,
                &TRACKINGTOGGLE_CMD,
                &TRACKINGSTATS_CMD,
                &TRACKINGCOOLDOWN_CMD,
                &TRACKINGINTERVAL_CMD,
            ],
        ),
    ]
}
