use crate::{
    arguments::Stream,
    core::{Command, CommandGroups},
    database::Prefix,
    util::CowUtils,
};

use std::borrow::Cow;

#[derive(Debug)]
pub enum Invoke {
    Command {
        cmd: &'static Command,
        num: Option<usize>,
    },
    SubCommand {
        main: &'static Command,
        sub: &'static Command,
    },
    Help(Option<&'static Command>),
    FailedHelp(String),
    None,
}

impl Invoke {
    #[inline]
    pub fn name(&self) -> Cow<str> {
        match self {
            Invoke::Command { cmd, .. } => Cow::Borrowed(cmd.names[0]),
            Invoke::SubCommand { main, sub } => {
                Cow::Owned(format!("{}-{}", main.names[0], sub.names[0]))
            }
            Invoke::Help(_) | Invoke::FailedHelp(_) => Cow::Borrowed("help"),
            Invoke::None => Cow::default(),
        }
    }
}

pub fn find_prefix<'a>(prefixes: &[Prefix], stream: &mut Stream<'a>) -> bool {
    prefixes.iter().any(|p| {
        if stream.starts_with(p) {
            stream.increment(p.len());

            true
        } else {
            false
        }
    })
}

pub fn parse_invoke(stream: &mut Stream<'_>, groups: &CommandGroups) -> Invoke {
    let mut name = stream
        .take_until_char(|c| c.is_whitespace() || c.is_numeric())
        .cow_to_ascii_lowercase();

    let num_str = stream.take_while_char(char::is_numeric);

    let num = if num_str.is_empty() {
        None
    } else if name.is_empty() {
        name = Cow::Borrowed(num_str);

        None
    } else {
        let n = num_str.chars().fold(0_usize, |n, c| {
            n.wrapping_mul(10).wrapping_add((c as u8 & 0xF) as usize)
        });

        Some(n)
    };

    stream.take_while_char(char::is_whitespace);

    match name.as_ref() {
        "h" | "help" => {
            let name = stream
                .take_until_char(char::is_whitespace)
                .cow_to_ascii_lowercase();

            stream.take_while_char(char::is_whitespace);

            if name.is_empty() {
                Invoke::Help(None)
            } else if let Some(cmd) = groups.get(name.as_ref()) {
                Invoke::Help(Some(cmd))
            } else {
                Invoke::FailedHelp(name.into_owned())
            }
        }
        _ => {
            if let Some(cmd) = groups.get(name.as_ref()) {
                let name = stream
                    .peek_until_char(|c| c.is_whitespace())
                    .cow_to_ascii_lowercase();

                for sub_cmd in cmd.sub_commands {
                    if sub_cmd.names.contains(&name.as_ref()) {
                        stream.increment(name.chars().count());
                        stream.take_while_char(char::is_whitespace);

                        return Invoke::SubCommand {
                            main: cmd,
                            sub: sub_cmd,
                        };
                    }
                }

                Invoke::Command { cmd, num }
            } else {
                Invoke::None
            }
        }
    }
}
