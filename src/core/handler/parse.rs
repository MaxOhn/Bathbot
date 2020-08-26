use crate::core::{Command, CommandGroups};

use std::borrow::Cow;
use uwl::Stream;

#[derive(Debug)]
pub enum Invoke {
    Command(&'static Command),
    SubCommand {
        main: &'static Command,
        sub: &'static Command,
    },
    Help(Option<&'static Command>),
    FailedHelp(String),
    None,
}

impl Invoke {
    pub fn name(&self) -> Cow<str> {
        match self {
            Invoke::Command(cmd) => Cow::Borrowed(cmd.names[0]),
            Invoke::SubCommand { main, sub } => {
                Cow::Owned(format!("{}-{}", main.names[0], sub.names[0]))
            }
            Invoke::Help(_) | Invoke::FailedHelp(_) => Cow::Borrowed("help"),
            Invoke::None => Cow::default(),
        }
    }
}

pub fn find_prefix<'a>(prefixes: &[String], stream: &mut Stream<'a>) -> bool {
    let prefix = prefixes.iter().find_map(|p| {
        let peeked = stream.peek_for_char(p.chars().count());
        if p == peeked {
            Some(peeked)
        } else {
            None
        }
    });
    if let Some(prefix) = &prefix {
        stream.advance_char(prefix.chars().count());
    }
    prefix.is_some()
}

pub fn parse_invoke(stream: &mut Stream<'_>, groups: &CommandGroups) -> Invoke {
    let name = stream.peek_until_char(|c| c.is_whitespace()).to_lowercase();
    stream.increment(name.len());
    stream.take_while_char(|c| c.is_whitespace());
    match name.as_str() {
        "h" | "help" => {
            let name = stream.peek_until_char(|c| c.is_whitespace()).to_lowercase();
            stream.increment(name.chars().count());
            stream.take_while_char(|c| c.is_whitespace());
            if name.is_empty() {
                Invoke::Help(None)
            } else if let Some(cmd) = groups.get(name.as_str()) {
                Invoke::Help(Some(cmd))
            } else {
                Invoke::FailedHelp(name)
            }
        }
        _ => {
            if let Some(cmd) = groups.get(name.as_str()) {
                let name = stream.peek_until_char(|c| c.is_whitespace()).to_lowercase();
                for sub_cmd in cmd.sub_commands {
                    if sub_cmd.names.contains(&name.as_str()) {
                        stream.increment(name.chars().count());
                        stream.take_while_char(|c| c.is_whitespace());
                        return Invoke::SubCommand {
                            main: cmd,
                            sub: sub_cmd,
                        };
                    }
                }
                Invoke::Command(cmd)
            } else {
                Invoke::None
            }
        }
    }
}
