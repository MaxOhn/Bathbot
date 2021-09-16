use super::Stream;
use crate::{util::matcher, BotResult, Context, Name};

use std::{error::Error, fmt};
use twilight_model::id::UserId;

pub struct Args<'m> {
    msg: &'m str,
    stream: Stream<'m>,
}

impl<'m> Iterator for Args<'m> {
    type Item = &'m str;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, end) = self.lex()?;

        Some(&self.msg[start..end])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = match self.stream.current_char() {
            Some(c) => !c.is_whitespace() as usize,
            None => 0,
        };

        let upper = self.stream.rest().split_whitespace().count();

        (lower, Some(upper))
    }
}

impl<'m> Args<'m> {
    pub fn new(msg: &'m str, stream: Stream<'m>) -> Self {
        Self { msg, stream }
    }

    pub fn is_empty(&self) -> bool {
        self.stream.is_empty()
    }

    pub fn rest(&self) -> &'m str {
        self.stream.rest()
    }

    fn lex(&mut self) -> Option<(usize, usize)> {
        let stream = &mut self.stream;
        let start = stream.offset();

        if stream.next()? == b'"' {
            stream.take_until(|b| b == b'"');
            let is_quote = stream.next().map_or(false, |b| b == b'"');
            let end = stream.offset();

            if start == end - 2 {
                stream.take_while_char(char::is_whitespace);

                return self.lex();
            }

            stream.take_while_char(char::is_whitespace);

            let limits = if is_quote {
                (start + 1, end - 1)
            } else {
                (start, stream.len())
            };

            return Some(limits);
        }

        stream.take_until_char(char::is_whitespace);
        let end = stream.offset();
        stream.take_while_char(char::is_whitespace);

        Some((start, end))
    }

    pub async fn check_user_mention(
        ctx: &Context,
        arg: &str,
    ) -> BotResult<Result<Name, &'static str>> {
        match matcher::get_mention_user(arg) {
            Some(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                Some(name) => Ok(Ok(name)),
                None => Ok(Err("The specified user is not linked to an osu profile")),
            },
            None => Ok(Ok(arg.into())),
        }
    }
}

#[derive(Debug)]
pub struct ArgError<E>(E);

impl<E> From<E> for ArgError<E> {
    fn from(e: E) -> Self {
        Self(e)
    }
}

impl<E: fmt::Display> fmt::Display for ArgError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<E: fmt::Debug + fmt::Display> Error for ArgError<E> {}
