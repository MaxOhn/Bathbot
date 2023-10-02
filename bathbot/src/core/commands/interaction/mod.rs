use std::{iter::Copied, pin::Pin};

use eyre::Result;
use futures::Future;
use once_cell::sync::OnceCell;
use radix_trie::{iter::Keys, Trie, TrieCommon};
use twilight_model::application::command::Command;

pub use self::command::{InteractionCommandKind, MessageCommand, SlashCommand};
#[cfg(feature = "osutracking")]
use crate::commands::tracking::*;
#[cfg(feature = "twitch")]
use crate::commands::twitch::*;
use crate::commands::{fun::*, help::*, osu::*, owner::*, songs::*, utility::*};

mod command;

macro_rules! interaction_trie {
    ( $( $( #[$meta:meta] )? $front:ident $( => $back:ident )? ,)* ) => {
            use twilight_interactions::command::CreateCommand;

            let mut trie = Trie::new();

            $( interaction_trie!(trie, $( #[$meta] )? $front $( => $back)? ) ;)*

            InteractionCommands(trie)
    };
    ($trie:ident, $( #[$meta:meta] )? $cmd:ident => $fun:ident ) => {
        $( #[$meta] )?
        $trie.insert($cmd::NAME, InteractionCommandKind::Chat(&$fun));
    };
    ($trie:ident, $( #[$meta:meta] )? $msg_cmd:ident ) => {
        $( #[$meta] )?
        $trie.insert($msg_cmd.name, InteractionCommandKind::Message(&$msg_cmd));
    };
}

static INTERACTION_COMMANDS: OnceCell<InteractionCommands> = OnceCell::new();

pub struct InteractionCommands(Trie<&'static str, InteractionCommandKind>);

pub type CommandResult = Pin<Box<dyn Future<Output = Result<()>> + 'static + Send>>;

type CommandKeys<'t> = Copied<Keys<'t, &'static str, InteractionCommandKind>>;

impl InteractionCommands {
    pub fn get() -> &'static Self {
        INTERACTION_COMMANDS.get_or_init(|| {
            interaction_trie! {
                Avatar => AVATAR_SLASH,
                Attributes => ATTRIBUTES_SLASH,
                Badges => BADGES_SLASH,
                Bg => BG_SLASH,
                Bookmarks => BOOKMARKS_SLASH,
                BOOKMARK_MAP_MSG,
                Bws => BWS_SLASH,
                Card => CARD_SLASH,
                Changelog => CHANGELOG_SLASH,
                ClaimName => CLAIMNAME_SLASH,
                Commands => COMMANDS_SLASH,
                Compare => COMPARE_SLASH,
                CompareScore_ => COMPARESCORE__SLASH,
                Config => CONFIG_SLASH,
                CountryTop => COUNTRYTOP_SLASH,
                Cp => CP_SLASH,
                Cs => CS_SLASH,
                Ct => CT_SLASH,
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
                Render => RENDER_SLASH,
                Roll => ROLL_SLASH,
                Scores => SCORES_SLASH,
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

    pub fn command(&self, command: &str) -> Option<InteractionCommandKind> {
        self.0.get(command).copied()
    }

    pub fn collect(&self) -> Vec<Command> {
        self.0
            .values()
            .map(InteractionCommandKind::create)
            .collect()
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
