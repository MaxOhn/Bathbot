use std::pin::Pin;

use futures::Future;
use radix_trie::{Trie, TrieCommon};

use crate::{
    commands::{fun::*, help::HELP_PREFIX, osu::*, songs::*, tracking::*, twitch::*, utility::*},
    util::Emote,
    BotResult,
};

pub use self::{args::Args, command::PrefixCommand, stream::Stream};

mod args;
mod command;
mod stream;

macro_rules! prefix_trie {
    ($($cmd:ident,)*) => {
        let mut trie = Trie::new();

        $(
            for &name in $cmd.names {
                if trie.insert(name, &$cmd).is_some() {
                    panic!("duplicate prefix command `{name}`");
                }
            }
        )*

        PrefixCommands(trie)
    }
}

lazy_static::lazy_static! {
    pub static ref PREFIX_COMMANDS: PrefixCommands = {
        prefix_trie! {
            ADDSTREAM_PREFIX,
            AUTHORITIES_PREFIX,
            AVATAR_PREFIX,
            BACKGROUNDGAME_PREFIX,
            BELGIANLEADERBOARD_PREFIX,
            BOMBSAWAY_PREFIX,
            BWS_PREFIX,
            CATCHIT_PREFIX,
            COMMANDS_PREFIX,
            COMMON_PREFIX,
            COMMONCTB_PREFIX,
            COMMONMANIA_PREFIX,
            COMMONTAIKO_PREFIX,
            COMPARE_PREFIX,
            COUNTRYRANKING_PREFIX,
            COUNTRYRANKINGCTB_PREFIX,
            COUNTRYRANKINGMANIA_PREFIX,
            COUNTRYRANKINGTAIKO_PREFIX,
            COUNTRYSNIPELIST_PREFIX,
            COUNTRYSNIPESTATS_PREFIX,
            CTB_PREFIX,
            DING_PREFIX,
            FIREANDFLAMES_PREFIX,
            FIREFLIES_PREFIX,
            FIX_PREFIX,
            FLAMINGO_PREFIX,
            INVITE_PREFIX,
            HELP_PREFIX,
            LEADERBOARD_PREFIX,
            LINK_PREFIX,
            MANIA_PREFIX,
            MAP_PREFIX,
            MAPPER_PREFIX,
            MAPPERCTB_PREFIX,
            MAPPERMANIA_PREFIX,
            MAPPERTAIKO_PREFIX,
            MATCHCOSTS_PREFIX,
            MATCHLIVE_PREFIX,
            MATCHLIVEREMOVE_PREFIX,
            MEDAL_PREFIX,
            MEDALRECENT_PREFIX,
            MEDALSCOMMON_PREFIX,
            MEDALSMISSING_PREFIX,
            MEDALSTATS_PREFIX,
            MINESWEEPER_PREFIX,
            MOSTPLAYED_PREFIX,
            MOSTPLAYEDCOMMON_PREFIX,
            NOCHOKES_PREFIX,
            NOCHOKESCTB_PREFIX,
            NOCHOKESTAIKO_PREFIX,
            OSU_PREFIX,
            OSUCOMPARE_PREFIX,
            OSUCOMPARECTB_PREFIX,
            OSUCOMPAREMANIA_PREFIX,
            OSUCOMPARETAIKO_PREFIX,
            OSUSTATSCOUNT_PREFIX,
            OSUSTATSCOUNTCTB_PREFIX,
            OSUSTATSCOUNTMANIA_PREFIX,
            OSUSTATSCOUNTTAIKO_PREFIX,
            OSUSTATSGLOBALS_PREFIX,
            OSUSTATSGLOBALSCTB_PREFIX,
            OSUSTATSGLOBALSMANIA_PREFIX,
            OSUSTATSGLOBALSTAIKO_PREFIX,
            OSUSTATSLIST_PREFIX,
            OSUSTATSLISTCTB_PREFIX,
            OSUSTATSLISTMANIA_PREFIX,
            OSUSTATSLISTTAIKO_PREFIX,
            PING_PREFIX,
            PLAYERSNIPELIST_PREFIX,
            PLAYERSNIPESTATS_PREFIX,
            PP_PREFIX,
            PPCTB_PREFIX,
            PPMANIA_PREFIX,
            PPTAIKO_PREFIX,
            PPRANKING_PREFIX,
            PPRANKINGCTB_PREFIX,
            PPRANKINGMANIA_PREFIX,
            PPRANKINGTAIKO_PREFIX,
            PREFIX_PREFIX,
            PRETENDER_PREFIX,
            PRUNE_PREFIX,
            RANK_PREFIX,
            RANKCTB_PREFIX,
            RANKMANIA_PREFIX,
            RANKTAIKO_PREFIX,
            RANKEDSCORERANKING_PREFIX,
            RANKEDSCORERANKINGCTB_PREFIX,
            RANKEDSCORERANKINGMANIA_PREFIX,
            RANKEDSCORERANKINGTAIKO_PREFIX,
            RANKRANKEDSCORE_PREFIX,
            RANKRANKEDSCORECTB_PREFIX,
            RANKRANKEDSCOREMANIA_PREFIX,
            RANKRANKEDSCORETAIKO_PREFIX,
            RATIOS_PREFIX,
            RECENT_PREFIX,
            RECENTCTB_PREFIX,
            RECENTMANIA_PREFIX,
            RECENTTAIKO_PREFIX,
            RECENTBEST_PREFIX,
            RECENTBESTCTB_PREFIX,
            RECENTBESTMANIA_PREFIX,
            RECENTBESTTAIKO_PREFIX,
            RECENTBELGIANLEADERBOARD_PREFIX,
            RECENTCTBBELGIANLEADERBOARD_PREFIX,
            RECENTMANIABELGIANLEADERBOARD_PREFIX,
            RECENTTAIKOBELGIANLEADERBOARD_PREFIX,
            RECENTLEADERBOARD_PREFIX,
            RECENTCTBLEADERBOARD_PREFIX,
            RECENTMANIALEADERBOARD_PREFIX,
            RECENTTAIKOLEADERBOARD_PREFIX,
            RECENTLIST_PREFIX,
            RECENTLISTCTB_PREFIX,
            RECENTLISTMANIA_PREFIX,
            RECENTLISTTAIKO_PREFIX,
            REMOVESTREAM_PREFIX,
            ROCKEFELLER_PREFIX,
            ROLEASSIGN_PREFIX,
            ROLL_PREFIX,
            SAYGOODBYE_PREFIX,
            SEARCH_PREFIX,
            SIMULATE_PREFIX,
            SIMULATERECENT_PREFIX,
            SIMULATERECENTCTB_PREFIX,
            SIMULATERECENTMANIA_PREFIX,
            SIMULATERECENTTAIKO_PREFIX,
            SNIPED_PREFIX,
            SNIPEDGAIN_PREFIX,
            SNIPEDLOSS_PREFIX,
            SOTARKS_PREFIX,
            STARTAGAIN_PREFIX,
            TAIKO_PREFIX,
            TIJDMACHINE_PREFIX,
            TOP_PREFIX,
            TOPCTB_PREFIX,
            TOPMANIA_PREFIX,
            TOPTAIKO_PREFIX,
            TOPIF_PREFIX,
            TOPIFCTB_PREFIX,
            TOPIFTAIKO_PREFIX,
            TOPOLD_PREFIX,
            TOPOLDCTB_PREFIX,
            TOPOLDMANIA_PREFIX,
            TOPOLDTAIKO_PREFIX,
            TRACK_PREFIX,
            TRACKCTB_PREFIX,
            TRACKMANIA_PREFIX,
            TRACKTAIKO_PREFIX,
            TRACKEDSTREAMS_PREFIX,
            TRACKLIST_PREFIX,
            UNTRACK_PREFIX,
            UNTRACKALL_PREFIX,
            WHATIF_PREFIX,
            WHATIFCTB_PREFIX,
            WHATIFMANIA_PREFIX,
            WHATIFTAIKO_PREFIX,
        }
    };
}

pub type CommandResult<'fut> = Pin<Box<dyn Future<Output = BotResult<()>> + 'fut + Send>>;

type PrefixTrie = Trie<&'static str, &'static PrefixCommand>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrefixCommandGroup {
    AllModes,
    Osu,
    Taiko,
    Catch,
    Mania,
    Tracking,
    Twitch,
    Games,
    Utility,
    Songs,
}

impl PrefixCommandGroup {
    pub fn emote(self) -> &'static str {
        match self {
            PrefixCommandGroup::AllModes => Emote::Osu.text(),
            PrefixCommandGroup::Osu => Emote::Std.text(),
            PrefixCommandGroup::Taiko => Emote::Tko.text(),
            PrefixCommandGroup::Catch => Emote::Ctb.text(),
            PrefixCommandGroup::Mania => Emote::Mna.text(),
            PrefixCommandGroup::Tracking => Emote::Tracking.text(),
            PrefixCommandGroup::Twitch => Emote::Twitch.text(),
            PrefixCommandGroup::Games => ":video_game:",
            PrefixCommandGroup::Utility => ":tools:",
            PrefixCommandGroup::Songs => ":musical_note:",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PrefixCommandGroup::AllModes => "all osu! modes",
            PrefixCommandGroup::Osu => "osu!standard",
            PrefixCommandGroup::Taiko => "osu!taiko",
            PrefixCommandGroup::Catch => "osu!catch",
            PrefixCommandGroup::Mania => "osu!mania",
            PrefixCommandGroup::Tracking => "osu!tracking",
            PrefixCommandGroup::Twitch => "twitch",
            PrefixCommandGroup::Games => "games",
            PrefixCommandGroup::Utility => "utility",
            PrefixCommandGroup::Songs => "songs",
        }
    }
}

pub struct PrefixCommands(PrefixTrie);

impl PrefixCommands {
    /// Access prefix commands so their lazy_static executes
    pub fn init(&self) {}

    pub fn command(&self, command: &str) -> Option<&'static PrefixCommand> {
        self.0.get(command).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static PrefixCommand> + '_ {
        self.0.values().copied()
    }
}
