use crate::{commands::command_groups, Args, BotResult, Context};

use futures::future::BoxFuture;
use radix_trie::Trie;
use std::{fmt, sync::Arc};
use twilight_model::channel::Message;

type CommandTree = Trie<&'static str, &'static Command>;
type BoxResult<'fut> = BoxFuture<'fut, BotResult<()>>;

pub struct Command {
    pub names: &'static [&'static str],
    pub short_desc: &'static str,
    pub long_desc: Option<&'static str>,
    pub usage: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub authority: bool,
    pub owner: bool,
    pub only_guilds: bool,
    pub bucket: Option<&'static str>,
    pub sub_commands: &'static [&'static Command],
    pub fun:
        for<'fut> fn(Arc<Context>, &'fut Message, Args<'fut>, Option<usize>) -> BoxResult<'fut>,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("names", &self.names)
            .field("short_desc", &self.short_desc)
            .field("long_desc", &self.long_desc)
            .field("usage", &self.usage)
            .field("examples", &self.examples)
            .field("sub_commands", &self.sub_commands)
            .finish()
    }
}

pub struct CommandGroup {
    pub name: String,
    pub commands: Vec<&'static Command>,
}

impl CommandGroup {
    pub fn new(name: &str, commands: Vec<&'static Command>) -> Self {
        Self {
            name: name.to_owned(),
            commands,
        }
    }
}

pub struct CommandGroups {
    pub groups: Vec<CommandGroup>,
    trie: CommandTree,
}

impl CommandGroups {
    pub fn new() -> Self {
        let groups = command_groups();
        let mut trie = Trie::new();
        for group in groups.iter() {
            for &cmd in group.commands.iter() {
                for &name in cmd.names {
                    if let Some(value) = trie.insert(name, cmd) {
                        panic!(
                            "Tried to insert command `{}` for `{}` but name already inserted for `{}`",
                            name, cmd.names[0], value.names[0]
                        );
                    }
                }
            }
        }
        Self { groups, trie }
    }

    pub fn get(&self, command: &str) -> Option<&'static Command> {
        self.trie.get(command).copied()
    }
}
