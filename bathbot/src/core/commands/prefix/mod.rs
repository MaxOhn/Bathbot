use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    pin::Pin,
};

use eyre::Result;
use futures::Future;
use linkme::distributed_slice;
use once_cell::sync::OnceCell;
use radix_trie::{Trie, TrieCommon};

pub use self::{
    args::{Args, ArgsNum},
    command::PrefixCommand,
};
use crate::util::Emote;

mod args;
mod command;

#[distributed_slice]
pub static __PREFIX_COMMANDS: [PrefixCommand] = [..];

static PREFIX_COMMANDS: OnceCell<PrefixCommands> = OnceCell::new();

pub type CommandResult<'fut> = Pin<Box<dyn Future<Output = Result<()>> + 'fut + Send>>;

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
    pub fn emote(self) -> PrefixCommandGroupEmote {
        PrefixCommandGroupEmote { group: self }
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

pub struct PrefixCommandGroupEmote {
    group: PrefixCommandGroup,
}

impl Display for PrefixCommandGroupEmote {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.group {
            PrefixCommandGroup::AllModes => Display::fmt(&Emote::Osu, f),
            PrefixCommandGroup::Osu => Display::fmt(&Emote::Std, f),
            PrefixCommandGroup::Taiko => Display::fmt(&Emote::Tko, f),
            PrefixCommandGroup::Catch => Display::fmt(&Emote::Ctb, f),
            PrefixCommandGroup::Mania => Display::fmt(&Emote::Mna, f),
            PrefixCommandGroup::Tracking => Display::fmt(&Emote::Tracking, f),
            PrefixCommandGroup::Twitch => Display::fmt(&Emote::Twitch, f),
            PrefixCommandGroup::Games => f.write_str(":video_game:"),
            PrefixCommandGroup::Utility => f.write_str(":tools:"),
            PrefixCommandGroup::Songs => f.write_str(":musical_note:"),
        }
    }
}

pub struct PrefixCommands(Trie<&'static str, &'static PrefixCommand>);

impl PrefixCommands {
    pub fn get() -> &'static Self {
        PREFIX_COMMANDS.get_or_init(|| {
            let mut trie = Trie::new();

            for cmd in __PREFIX_COMMANDS {
                for &name in cmd.names {
                    if trie.insert(name, cmd).is_some() {
                        panic!("duplicate prefix command `{name}`");
                    }
                }
            }

            PrefixCommands(trie)
        })
    }

    pub fn command(&self, command: &str) -> Option<&'static PrefixCommand> {
        self.0.get(command).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static PrefixCommand> + '_ {
        self.0.values().copied()
    }
}
