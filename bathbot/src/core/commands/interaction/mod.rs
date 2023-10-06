use std::{iter::Copied, pin::Pin};

use eyre::Result;
use futures::Future;
use linkme::distributed_slice;
use once_cell::sync::OnceCell;
use radix_trie::{iter::Keys, Trie, TrieCommon};
use twilight_model::application::command::Command;

pub use self::command::{InteractionCommandKind, MessageCommand, SlashCommand};

mod command;

#[distributed_slice]
pub static __SLASH_COMMANDS: [SlashCommand] = [..];

#[distributed_slice]
pub static __MSG_COMMANDS: [MessageCommand] = [..];

static INTERACTION_COMMANDS: OnceCell<InteractionCommands> = OnceCell::new();

pub struct InteractionCommands(Trie<&'static str, InteractionCommandKind>);

pub type CommandResult = Pin<Box<dyn Future<Output = Result<()>> + 'static + Send>>;

type CommandKeys<'t> = Copied<Keys<'t, &'static str, InteractionCommandKind>>;

impl InteractionCommands {
    pub fn get() -> &'static Self {
        INTERACTION_COMMANDS.get_or_init(|| {
            let mut trie = Trie::new();

            for cmd in __SLASH_COMMANDS {
                trie.insert(cmd.name, InteractionCommandKind::Chat(cmd));
            }

            for cmd in __MSG_COMMANDS {
                trie.insert(cmd.name, InteractionCommandKind::Message(cmd));
            }

            InteractionCommands(trie)
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
