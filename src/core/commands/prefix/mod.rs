use std::{iter::Copied, pin::Pin};

use futures::Future;
use radix_trie::{iter::Keys, Trie, TrieCommon};

use crate::{
    commands::{fun::*, help::HELP_PREFIX, osu::*, songs::*, tracking::*, twitch::*, utility::*},
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
            DING_PREFIX,
            FIREANDFLAMES_PREFIX,
            FIREFLIES_PREFIX,
            FIX_PREFIX,
            FLAMINGO_PREFIX,
            INVITE_PREFIX,
            HELP_PREFIX,
            LEADERBOARD_PREFIX,
            LINK_PREFIX,
            MAP_PREFIX,
            MAPPER_PREFIX,
            MAPPERCTB_PREFIX,
            MAPPERMANIA_PREFIX,
            MAPPERTAIKO_PREFIX,
            MATCHLIVE_PREFIX,
            MINESWEEPER_PREFIX,
            MOSTPLAYED_PREFIX,
            PING_PREFIX,
            PP_PREFIX,
            PPCTB_PREFIX,
            PPMANIA_PREFIX,
            PPTAIKO_PREFIX,
            PREFIX_PREFIX,
            PRETENDER_PREFIX,
            PRUNE_PREFIX,
            RATIOS_PREFIX,
            RECENTBEST_PREFIX,
            RECENTBESTCTB_PREFIX,
            RECENTBESTMANIA_PREFIX,
            RECENTBESTTAIKO_PREFIX,
            REMOVESTREAM_PREFIX,
            ROCKEFELLER_PREFIX,
            ROLEASSIGN_PREFIX,
            ROLL_PREFIX,
            SEARCH_PREFIX,
            SOTARKS_PREFIX,
            STARTAGAIN_PREFIX,
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

type CommandKeys<'t> = Copied<Keys<'t, &'static str, &'static PrefixCommand>>;
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
    Owner,
}

impl PrefixCommandGroup {
    pub fn emote(self) -> &'static str {
        "group emote"
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
            PrefixCommandGroup::Owner => "owner",
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

    // TODO: No need to .collect()
    pub fn collect(&self) -> Vec<&'static PrefixCommand> {
        self.0.values().copied().collect()
    }
}
