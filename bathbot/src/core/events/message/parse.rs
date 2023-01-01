use std::borrow::Cow;

use crate::{
    core::commands::prefix::{PrefixCommand, PrefixCommands, Stream},
    util::CowUtils,
};

pub enum Invoke {
    Command {
        cmd: &'static PrefixCommand,
        num: Option<u64>,
    },
    None,
}

pub fn parse_invoke(stream: &mut Stream<'_>) -> Invoke {
    let mut name = stream
        .take_until_char(|c| c.is_whitespace() || c.is_numeric())
        .cow_to_ascii_lowercase();

    let num_str = stream.take_while_char(char::is_numeric);

    let num = if num_str.is_empty() {
        None
    } else if name.is_empty() {
        name = Cow::Borrowed(num_str);

        // thing like <rb1badewanne3 don't need to be considered
        if stream.take_while_char(char::is_whitespace).is_empty() && !stream.is_empty() {
            return Invoke::None;
        }

        None
    } else {
        // Efficient integer parsing
        let n = num_str.chars().fold(0_u64, |n, c| {
            n.wrapping_mul(10).wrapping_add((c as u8 & 0xF) as u64)
        });

        if stream.take_while_char(char::is_whitespace).is_empty() && !stream.is_empty() {
            return Invoke::None;
        }

        Some(n)
    };

    stream.take_while_char(char::is_whitespace);

    if let Some(cmd) = PrefixCommands::get().command(name.as_ref()) {
        Invoke::Command { cmd, num }
    } else {
        Invoke::None
    }
}
