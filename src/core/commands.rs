use crate::commands::command_groups;

use radix_trie::Trie;
use std::ops::Deref;

pub struct Command {
    pub names: &'static [&'static str],
    pub short_desc: &'static str,
    pub long_desc: Option<&'static str>,
    pub usage: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub sub_commands: &'static [&'static Command],
    pub fun: fn(&mut (), &(), ()) -> (),
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
    trie: Trie<&'static str, &'static Command>,
}

impl CommandGroups {
    pub fn new() -> Self {
        let groups = command_groups();
        let mut trie = Trie::new();
        for group in groups.iter() {
            for &cmd in group.commands.iter() {
                for &name in cmd.names {
                    trie.insert(name, cmd);
                }
            }
        }
        Self { groups, trie }
    }
}

impl Deref for CommandGroups {
    type Target = Trie<&'static str, &'static Command>;

    fn deref(&self) -> &Self::Target {
        &self.trie
    }
}
