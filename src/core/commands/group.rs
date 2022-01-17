use super::Command;
use crate::util::Emote;

use radix_trie::Trie;

type CommandTree = Trie<&'static str, &'static Command>;

pub struct CommandGroup {
    pub name: String,
    pub commands: Vec<&'static Command>,
    pub emote: Emote,
}

impl CommandGroup {
    pub fn new(name: &str, emote: Emote, commands: Vec<&'static Command>) -> Self {
        Self {
            name: name.to_owned(),
            commands,
            emote,
        }
    }
}

pub struct CommandGroups {
    pub groups: [CommandGroup; 11],
    trie: CommandTree,
}

lazy_static! {
    pub static ref CMD_GROUPS: CommandGroups = {
        let groups = crate::commands::command_groups();
        let mut trie = Trie::new();

        for group in groups.iter() {
            for &cmd in group.commands.iter() {
                for &name in cmd.names {
                    if let Some(value) = trie.insert(name, cmd) {
                        panic!(
                            "Tried to insert command `{name}` for `{}` but name already inserted for `{}`",
                            cmd.names[0], value.names[0]
                        );
                    }
                }
            }
        }

        CommandGroups { groups, trie }
    };
}

impl CommandGroups {
    pub fn get(&self, command: &str) -> Option<&'static Command> {
        self.trie.get(command).copied()
    }
}
