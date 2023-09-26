use bathbot_util::CowUtils;
use nom::{
    branch::alt,
    character::complete as ch,
    combinator::{eof, map, map_opt, opt, recognize},
    sequence::{pair, terminated},
};

use crate::core::commands::prefix::{Args, ArgsNum, PrefixCommand, PrefixCommands};

pub struct Invoke<'i> {
    pub cmd: &'static PrefixCommand,
    pub args: Args<'i>,
}

impl<'i> Invoke<'i> {
    pub fn parse(input: &'i str) -> Option<Self> {
        let mut parse = terminated::<_, _, _, (), _, _>(
            // either
            alt((
                // [alphabetic][numeric/?]
                pair(
                    map_opt(ch::alpha1, |name: &str| {
                        PrefixCommands::get().command(name.cow_to_ascii_lowercase().as_ref())
                    }),
                    map(
                        opt(alt((
                            map(ch::u32, ArgsNum::Value),
                            map(ch::char('?'), |_| ArgsNum::Random),
                        ))),
                        |opt| opt.unwrap_or(ArgsNum::None),
                    ),
                ),
                // [numeric]
                map_opt(ch::digit1, |name| {
                    PrefixCommands::get()
                        .command(name)
                        .map(|cmd| (cmd, ArgsNum::None))
                }),
            )),
            // either followed by space or eof
            recognize(alt((ch::space1, eof))),
        );

        let (rest, (cmd, num)) = parse(input).ok()?;
        let args = Args::new(rest, num);

        Some(Self { cmd, args })
    }
}
