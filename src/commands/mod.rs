/// E.g: `bail_cmd_option!("link", subcommand, name)`
macro_rules! bail_cmd_option {
    ($cmd:expr, string, $name:ident) => {
        bail_cmd_option!(@ $cmd, "string", $name);
    };
    ($cmd:expr, integer, $name:ident) => {
        bail_cmd_option!(@ $cmd, "integer", $name);
    };
    ($cmd:expr, boolean, $name:ident) => {
        bail_cmd_option!(@ $cmd, "boolean", $name);
    };
    ($cmd:expr, subcommand, $name:ident) => {
        bail_cmd_option!(@ $cmd, "subcommand", $name);
    };
    ($cmd:expr, $any:tt, $name:ident) => {
       compile_error!("expected `string`, `integer`, `boolean`, or `subcommand` as second argument");
    };

    (@ $cmd:expr, $kind:literal, $name:ident) => {
        return Err(crate::Error::UnexpectedCommandOption {
            cmd: $cmd,
            kind: $kind,
            name: $name,
        })
    };
}

/// E.g: `parse_mode_option!(value, "recent score")`
macro_rules! parse_mode_option {
    ($value:ident, $location:literal) => {
        match $value.as_str() {
            "osu" => Some(GameMode::STD),
            "taiko" => Some(GameMode::TKO),
            "catch" => Some(GameMode::CTB),
            "mania" => Some(GameMode::MNA),
            _ => bail_cmd_option!(concat!($location, " mode"), string, $value),
        }
    };
}

/// E.g: `parse_discord_option!(ctx, value, "top rebalance")`
macro_rules! parse_discord_option {
    ($ctx:ident, $value:ident, $location:literal) => {
        match $value.parse() {
            Ok(id) => match $ctx.user_config(twilight_model::id::UserId(id)).await?.name {
                Some(name) => Some(name),
                None => {
                    let content = format!("<@{}> is not linked to an osu profile", id);

                    return Ok(Err(content.into()));
                }
            },
            Err(_) => bail_cmd_option!(concat!($location, " discord"), string, $value),
        }
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
                &COMPARE_CMD,
                &SIMULATE_CMD,
                &MAP_CMD,
                &FIX_CMD,
                &MATCHCOSTS_CMD,
                &AVATAR_CMD,
                &MOSTPLAYED_CMD,
                &MOSTPLAYEDCOMMON_CMD,
                &LEADERBOARD_CMD,
                &BELGIANLEADERBOARD_CMD,
                &MEDAL_CMD,
                &MEDALSTATS_CMD,
                &MEDALRECENT_CMD,
                &MEDALSMISSING_CMD,
                &MEDALSCOMMON_CMD,
                &SEARCH_CMD,
                &MATCHLIVE_CMD,
                &MATCHLIVEREMOVE_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!standard",
            Emote::Std,
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
                &BWS_CMD,
                &RECENTLEADERBOARD_CMD,
                &RECENTBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALS_CMD,
                &OSUSTATSCOUNT_CMD,
                &OSUSTATSLIST_CMD,
                &SIMULATERECENT_CMD,
                &RECENTLIST_CMD,
                &NOCHOKES_CMD,
                &SOTARKS_CMD,
                &MAPPER_CMD,
                &TOPIF_CMD,
                &TOPOLD_CMD,
                &REBALANCE_CMD,
                &SNIPED_CMD,
                &SNIPEDGAIN_CMD,
                &SNIPEDLOSS_CMD,
                &PLAYERSNIPESTATS_CMD,
                &PLAYERSNIPELIST_CMD,
                &COUNTRYSNIPESTATS_CMD,
                &COUNTRYSNIPELIST_CMD,
                &RANKRANKEDSCORE_CMD,
                &PPRANKING_CMD,
                &RANKEDSCORERANKING_CMD,
                &COUNTRYRANKING_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!mania",
            Emote::Mna,
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
                &RECENTLISTMANIA_CMD,
                &RATIOS_CMD,
                &MAPPERMANIA_CMD,
                &TOPOLDMANIA_CMD,
                &RANKRANKEDSCOREMANIA_CMD,
                &PPRANKINGMANIA_CMD,
                &RANKEDSCORERANKINGMANIA_CMD,
                &COUNTRYRANKINGMANIA_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!taiko",
            Emote::Tko,
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
                &RECENTLISTTAIKO_CMD,
                &NOCHOKESTAIKO_CMD,
                &MAPPERTAIKO_CMD,
                &TOPIFTAIKO_CMD,
                &TOPOLDTAIKO_CMD,
                &RANKRANKEDSCORETAIKO_CMD,
                &PPRANKINGTAIKO_CMD,
                &RANKEDSCORERANKINGTAIKO_CMD,
                &COUNTRYRANKINGTAIKO_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!catch the beat",
            Emote::Ctb,
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
                &RECENTLISTCTB_CMD,
                &NOCHOKESCTB_CMD,
                &MAPPERCTB_CMD,
                &TOPIFCTB_CMD,
                &TOPOLDCTB_CMD,
                &RANKRANKEDSCORECTB_CMD,
                &PPRANKINGCTB_CMD,
                &RANKEDSCORERANKINGCTB_CMD,
                &COUNTRYRANKINGCTB_CMD,
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
            vec![&MINESWEEPER_CMD, &BACKGROUNDGAME_CMD],
        ),
        CommandGroup::new(
            "utility",
            Emote::Custom("tools"),
            vec![
                &PING_CMD,
                &ROLL_CMD,
                &CONFIG_CMD,
                &ABOUT_CMD,
                &COMMANDS_CMD,
                &INVITE_CMD,
                &PRUNE_CMD,
                &PREFIX_CMD,
                &ECHO_CMD,
                &AUTHORITIES_CMD,
                &ROLEASSIGN_CMD,
                &TOGGLESONGS_CMD,
            ],
        ),
        CommandGroup::new(
            "songs",
            Emote::Custom("musical_note"),
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
                &STARTAGAIN_CMD,
                &TIJDMACHINE_CMD,
            ],
        ),
        CommandGroup::new(
            "owner",
            Emote::Custom("crown"),
            vec![
                &ADDBG_CMD,
                &ADDCOUNTRY_CMD,
                &CACHE_CMD,
                &ACTIVEBG_CMD,
                &BGTAGS_CMD,
                &BGTAGSMANUAL_CMD,
                &CHANGEGAME_CMD,
                &TRACKINGTOGGLE_CMD,
                &TRACKINGSTATS_CMD,
                &TRACKINGCOOLDOWN_CMD,
                &TRACKINGINTERVAL_CMD,
            ],
        ),
    ]
}

pub fn slash_commands() -> [Command; 39] {
    [
        help::slash_help_command(),
        slash_recent_command(),
        slash_compare_command(),
        slash_link_command(),
        slash_top_command(),
        slash_osustats_command(),
        slash_backgroundgame_command(),
        slash_profile_command(),
        slash_snipe_command(),
        slash_matchcost_command(),
        slash_roll_command(),
        slash_leaderboard_command(),
        slash_reach_command(),
        slash_whatif_command(),
        slash_map_command(),
        slash_bws_command(),
        slash_medal_command(),
        slash_track_command(),
        slash_mostplayed_command(),
        slash_ranking_command(),
        slash_ping_command(),
        slash_simulate_command(),
        slash_fix_command(),
        slash_config_command(),
        slash_mapsearch_command(),
        slash_ratio_command(),
        slash_trackstream_command(),
        slash_matchlive_command(),
        slash_invite_command(),
        slash_about_command(),
        slash_commands_command(),
        slash_avatar_command(),
        slash_song_command(),
        slash_minesweeper_command(),
        slash_prune_command(),
        slash_authorities_command(),
        slash_togglesongs_command(),
        slash_roleassign_command(),
        slash_owner_command(),
    ]
}
