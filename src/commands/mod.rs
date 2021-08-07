/// E.g: `bail_cmd_option!("link", subcommand, name)`
macro_rules! bail_cmd_option {
    ($cmd:literal, string, $name:ident) => {
        bail_cmd_option!(@ $cmd, "string", $name);
    };
    ($cmd:literal, integer, $name:ident) => {
        bail_cmd_option!(@ $cmd, "integer", $name);
    };
    ($cmd:literal, boolean, $name:ident) => {
        bail_cmd_option!(@ $cmd, "boolean", $name);
    };
    ($cmd:literal, subcommand, $name:ident) => {
        bail_cmd_option!(@ $cmd, "subcommand", $name);
    };
    ($cmd:literal, $any:tt, $name:ident) => {
       compile_error!("expected `string`, `integer`, `boolean`, or `subcommand` as second argument");
    };

    (@ $cmd:literal, $kind:literal, $name:ident) => {
        return Err(crate::Error::UnexpectedCommandOption {
            cmd: $cmd,
            kind: $kind,
            name: $name,
        })
    };
}

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

use crate::{core::CommandGroup, util::Emote};

use twilight_model::application::command::Command;

pub fn command_groups() -> [CommandGroup; 11] {
    [
        CommandGroup::new(
            "all osu! modes",
            Emote::Osu,
            vec![
                &LINK_CMD,
                // &COMPARE_CMD,
                // &SIMULATE_CMD,
                // &MAP_CMD,
                // &FIX_CMD,
                // &MATCHCOSTS_CMD,
                // &BWS_CMD,
                // &AVATAR_CMD,
                // &MOSTPLAYED_CMD,
                // &MOSTPLAYEDCOMMON_CMD,
                // &LEADERBOARD_CMD,
                // &BELGIANLEADERBOARD_CMD,
                // &MEDAL_CMD,
                // &MEDALSTATS_CMD,
                // &MEDALSMISSING_CMD,
                // &SEARCH_CMD,
                // &MATCHLIVE_CMD,
                // &MATCHLIVEREMOVE_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!standard",
            Emote::Std,
            vec![
                // &RECENT_CMD,
                // &TOP_CMD,
                // &RECENTBEST_CMD,
                // &OSU_CMD,
                // &OSUCOMPARE_CMD,
                // &PP_CMD,
                // &WHATIF_CMD,
                // &RANK_CMD,
                // &COMMON_CMD,
                // &RECENTLEADERBOARD_CMD,
                // &RECENTBELGIANLEADERBOARD_CMD,
                // &OSUSTATSGLOBALS_CMD,
                // &OSUSTATSCOUNT_CMD,
                // &OSUSTATSLIST_CMD,
                // &SIMULATERECENT_CMD,
                // &RECENTLIST_CMD,
                // &RECENTPAGES_CMD,
                // &NOCHOKES_CMD,
                // &SOTARKS_CMD,
                // &MAPPER_CMD,
                // &TOPIF_CMD,
                // &TOPOLD_CMD,
                // &REBALANCE_CMD,
                // &SNIPED_CMD,
                // &SNIPEDGAIN_CMD,
                // &SNIPEDLOSS_CMD,
                // &PLAYERSNIPESTATS_CMD,
                // &PLAYERSNIPELIST_CMD,
                // &COUNTRYSNIPESTATS_CMD,
                // &COUNTRYSNIPELIST_CMD,
                // &RANKRANKEDSCORE_CMD,
                // &PPRANKING_CMD,
                // &RANKEDSCORERANKING_CMD,
                // &COUNTRYRANKING_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!mania",
            Emote::Mna,
            vec![
                // &RECENTMANIA_CMD,
                // &TOPMANIA_CMD,
                // &RECENTBESTMANIA_CMD,
                // &MANIA_CMD,
                // &OSUCOMPAREMANIA_CMD,
                // &PPMANIA_CMD,
                // &WHATIFMANIA_CMD,
                // &RANKMANIA_CMD,
                // &COMMONMANIA_CMD,
                // &RECENTMANIALEADERBOARD_CMD,
                // &RECENTMANIABELGIANLEADERBOARD_CMD,
                // &OSUSTATSGLOBALSMANIA_CMD,
                // &OSUSTATSCOUNTMANIA_CMD,
                // &OSUSTATSLISTMANIA_CMD,
                // &SIMULATERECENTMANIA_CMD,
                // &RECENTLISTMANIA_CMD,
                // &RECENTPAGESMANIA_CMD,
                // &RATIOS_CMD,
                // &MAPPERMANIA_CMD,
                // &TOPOLDMANIA_CMD,
                // &RANKRANKEDSCOREMANIA_CMD,
                // &PPRANKINGMANIA_CMD,
                // &RANKEDSCORERANKINGMANIA_CMD,
                // &COUNTRYRANKINGMANIA_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!taiko",
            Emote::Tko,
            vec![
                // &RECENTTAIKO_CMD,
                // &TOPTAIKO_CMD,
                // &RECENTBESTTAIKO_CMD,
                // &TAIKO_CMD,
                // &OSUCOMPARETAIKO_CMD,
                // &PPTAIKO_CMD,
                // &WHATIFTAIKO_CMD,
                // &RANKTAIKO_CMD,
                // &COMMONTAIKO_CMD,
                // &RECENTTAIKOLEADERBOARD_CMD,
                // &RECENTTAIKOBELGIANLEADERBOARD_CMD,
                // &OSUSTATSGLOBALSTAIKO_CMD,
                // &OSUSTATSCOUNTTAIKO_CMD,
                // &OSUSTATSLISTTAIKO_CMD,
                // &SIMULATERECENTTAIKO_CMD,
                // &RECENTLISTTAIKO_CMD,
                // &RECENTPAGESTAIKO_CMD,
                // &NOCHOKESTAIKO_CMD,
                // &MAPPERTAIKO_CMD,
                // &TOPIFTAIKO_CMD,
                // &TOPOLDTAIKO_CMD,
                // &RANKRANKEDSCORETAIKO_CMD,
                // &PPRANKINGTAIKO_CMD,
                // &RANKEDSCORERANKINGTAIKO_CMD,
                // &COUNTRYRANKINGTAIKO_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!catch the beat",
            Emote::Ctb,
            vec![
                // &RECENTCTB_CMD,
                // &TOPCTB_CMD,
                // &RECENTBESTCTB_CMD,
                // &CTB_CMD,
                // &OSUCOMPARECTB_CMD,
                // &PPCTB_CMD,
                // &WHATIFCTB_CMD,
                // &RANKCTB_CMD,
                // &COMMONCTB_CMD,
                // &RECENTCTBLEADERBOARD_CMD,
                // &RECENTCTBBELGIANLEADERBOARD_CMD,
                // &OSUSTATSGLOBALSCTB_CMD,
                // &OSUSTATSCOUNTCTB_CMD,
                // &OSUSTATSLISTCTB_CMD,
                // &SIMULATERECENTCTB_CMD,
                // &RECENTLISTCTB_CMD,
                // &RECENTPAGESCTB_CMD,
                // &NOCHOKESCTB_CMD,
                // &MAPPERCTB_CMD,
                // &TOPIFCTB_CMD,
                // &TOPOLDCTB_CMD,
                // &RANKRANKEDSCORECTB_CMD,
                // &PPRANKINGCTB_CMD,
                // &RANKEDSCORERANKINGCTB_CMD,
                // &COUNTRYRANKINGCTB_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!tracking",
            Emote::Tracking,
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
            Emote::Twitch,
            vec![&ADDSTREAM_CMD, &REMOVESTREAM_CMD, &TRACKEDSTREAMS_CMD],
        ),
        CommandGroup::new(
            "games",
            Emote::Custom("video_game"),
            vec![
                &MINESWEEPER_CMD,
                // &BACKGROUNDGAME_CMD
            ],
        ),
        CommandGroup::new(
            "utility",
            Emote::Custom("tools"),
            vec![
                &PING_CMD,
                // &ROLL_CMD,
                // &ABOUT_CMD,
                // &COMMANDS_CMD,
                // &INVITE_CMD,
                // &PRUNE_CMD,
                // &PREFIX_CMD,
                // &ECHO_CMD,
                // &AUTHORITIES_CMD,
                // &ROLEASSIGN_CMD,
                // &TOGGLESONGS_CMD,
            ],
        ),
        CommandGroup::new(
            "songs",
            Emote::Custom("musical_note"),
            vec![
                // &BOMBSAWAY_CMD,
                // &CATCHIT_CMD,
                // &DING_CMD,
                // &FIREANDFLAMES_CMD,
                // &FIREFLIES_CMD,
                // &FLAMINGO_CMD,
                // &PRETENDER_CMD,
                // &ROCKEFELLER_CMD,
                // &SAYGOODBYE_CMD,
                // &TIJDMACHINE_CMD,
            ],
        ),
        CommandGroup::new(
            "owner",
            Emote::Custom("crown"),
            vec![
                // &ADDBG_CMD,
                // &ADDCOUNTRY_CMD,
                // &CACHE_CMD,
                // &ACTIVEBG_CMD,
                // &BGTAGS_CMD,
                // &BGTAGSMANUAL_CMD,
                // &CHANGEGAME_CMD,
                // &TRACKINGTOGGLE_CMD,
                // &TRACKINGSTATS_CMD,
                // &TRACKINGCOOLDOWN_CMD,
                // &TRACKINGINTERVAL_CMD,
            ],
        ),
    ]
}

// TODO: Make array when done
pub fn slash_commands() -> Vec<Command> {
    vec![
        slash_link_command(),
        slash_track_command(),
        slash_ping_command(),
        slash_trackstream_command(),
        slash_minesweeper_command(),
    ]
}
