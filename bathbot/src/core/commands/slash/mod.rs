use std::{iter::Copied, pin::Pin};

use eyre::Result;
use futures::Future;
use once_cell::sync::OnceCell;
use radix_trie::{iter::Keys, Trie, TrieCommon};
use twilight_model::application::command::Command;

use crate::commands::{fun::*, help::*, osu::*, owner::*, songs::*, utility::*};

#[cfg(feature = "osutracking")]
use crate::commands::tracking::*;

#[cfg(feature = "twitch")]
use crate::commands::twitch::*;

pub use self::command::SlashCommand;

mod command;

macro_rules! slash_trie {
    ( $( $( #[$meta:meta] )? $cmd:ident => $fun:ident ,)* ) => {
        use twilight_interactions::command::CreateCommand;

        let mut trie = Trie::new();

        $(
            $( #[$meta] )?
            trie.insert($cmd::NAME, &$fun);
        )*

        SlashCommands(trie)
    }
}

static SLASH_COMMANDS: OnceCell<SlashCommands> = OnceCell::new();

pub struct SlashCommands(Trie<&'static str, &'static SlashCommand>);

pub type CommandResult = Pin<Box<dyn Future<Output = Result<()>> + 'static + Send>>;

type CommandKeys<'t> = Copied<Keys<'t, &'static str, &'static SlashCommand>>;

impl SlashCommands {
    pub fn get() -> &'static Self {
        SLASH_COMMANDS.get_or_init(|| {
            slash_trie! {
                Avatar => AVATAR_SLASH,
                Attributes => ATTRIBUTES_SLASH,
                Badges => BADGES_SLASH,
                Bg => BG_SLASH,
                Bws => BWS_SLASH,
                Card => CARD_SLASH,
                ClaimName => CLAIMNAME_SLASH,
                Commands => COMMANDS_SLASH,
                Compare => COMPARE_SLASH,
                CompareScore_ => COMPARESCORE__SLASH,
                Config => CONFIG_SLASH,
                CountryTop => COUNTRYTOP_SLASH,
                Cs => CS_SLASH,
                Fix => FIX_SLASH,
                Graph => GRAPH_SLASH,
                Help => HELP_SLASH,
                HigherLower => HIGHERLOWER_SLASH,
                Invite => INVITE_SLASH,
                Leaderboard => LEADERBOARD_SLASH,
                #[cfg(feature = "server")]
                Link => LINK_SLASH,
                Map => MAP_SLASH,
                Mapper => MAPPER_SLASH,
                MatchCompare => MATCHCOMPARE_SLASH,
                MatchCost => MATCHCOST_SLASH,
                #[cfg(feature = "matchlive")]
                Matchlive => MATCHLIVE_SLASH,
                Medal => MEDAL_SLASH,
                Minesweeper => MINESWEEPER_SLASH,
                MostPlayed => MOSTPLAYED_SLASH,
                Nochoke => NOCHOKE_SLASH,
                Osc => OSC_SLASH,
                Osekai => OSEKAI_SLASH,
                OsuStats => OSUSTATS_SLASH,
                Owner => OWNER_SLASH,
                Ping => PING_SLASH,
                Pinned => PINNED_SLASH,
                Popular => POPULAR_SLASH,
                Pp => PP_SLASH,
                Profile => PROFILE_SLASH,
                Rank => RANK_SLASH,
                Ranking => RANKING_SLASH,
                Ratios => RATIOS_SLASH,
                Rb => RB_SLASH,
                Rs => RS_SLASH,
                Recent => RECENT_SLASH,
                Roll => ROLL_SLASH,
                Search => SEARCH_SLASH,
                ServerConfig => SERVERCONFIG_SLASH,
                ServerLeaderboard => SERVERLEADERBOARD_SLASH,
                Simulate => SIMULATE_SLASH,
                Skin => SKIN_SLASH,
                Snipe => SNIPE_SLASH,
                SnipePlayerSniped => SNIPEPLAYERSNIPED_SLASH,
                Song => SONG_SLASH,
                Top => TOP_SLASH,
                TopIf => TOPIF_SLASH,
                TopOld => TOPOLD_SLASH,
                #[cfg(feature = "osutracking")]
                Track => TRACK_SLASH,
                #[cfg(feature = "twitch")]
                TrackStream => TRACKSTREAM_SLASH,
                WhatIf => WHATIF_SLASH,
            }
        })
    }

    pub fn command(&self, command: &str) -> Option<&'static SlashCommand> {
        self.0.get(command).copied()
    }

    pub fn collect(&self) -> Vec<Command> {
        self.0.values().map(|c| (c.create)().into()).collect()
    }

    pub fn names(&self) -> CommandKeys<'_> {
        self.0.keys().copied()
    }

    pub fn descendants(&self, prefix: &str) -> Option<CommandKeys<'_>> {
        self.0
            .get_raw_descendant(prefix)
            .map(|sub| sub.keys().copied())
    }
}
