use std::{collections::VecDeque, error::Error, fmt, iter, str::FromStr};
use uwl::Stream;

pub struct Args<'m> {
    msg: &'m str,
    stream: Stream<'m>,
}

impl<'m> Iterator for Args<'m> {
    type Item = &'m str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (start, end) = self.lex()?;
        Some(&self.msg[start..end])
    }

    #[inline]
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
    #[inline]
    pub fn new(msg: &'m str, stream: Stream<'m>) -> Self {
        Self { msg, stream }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.stream.is_empty()
    }

    #[inline]
    pub fn rest(&self) -> &'m str {
        self.stream.rest()
    }

    #[inline]
    pub fn take_n(mut self, n: usize) -> ArgsFull<'m> {
        let limits = iter::from_fn(|| self.lex()).take(n).collect();
        ArgsFull {
            msg: self.msg,
            limits,
        }
    }

    #[inline]
    pub fn take_all(mut self) -> ArgsFull<'m> {
        let limits = iter::from_fn(|| self.lex()).collect();
        ArgsFull {
            msg: self.msg,
            limits,
        }
    }

    #[inline]
    pub fn single<T: FromStr>(&mut self) -> Result<T, ArgError<T::Err>> {
        let (start, end) = self.lex().ok_or(ArgError::Eos)?;
        let arg = &self.msg[start..end];
        T::from_str(arg).map_err(ArgError::Parse)
    }

    fn lex(&mut self) -> Option<(usize, usize)> {
        let stream = &mut self.stream;
        let start = stream.offset();
        if stream.current()? == b'"' {
            stream.next();
            stream.take_until(|b| b == b'"');
            let is_quote = stream.current().map_or(false, |b| b == b'"');
            stream.next();
            let end = stream.offset();
            if start == end - 2 {
                return self.lex();
            }
            stream.take_while_char(|c| c.is_whitespace());
            let limits = if is_quote {
                (start + 1, end - 1)
            } else {
                (start, stream.len())
            };
            return Some(limits);
        }
        stream.take_while_char(|c| !c.is_whitespace());
        let end = stream.offset();
        stream.take_while_char(|c| c.is_whitespace());
        Some((start, end))
    }
}

pub struct ArgsFull<'m> {
    msg: &'m str,
    limits: VecDeque<(usize, usize)>,
}

impl<'m> Iterator for ArgsFull<'m> {
    type Item = &'m str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (start, end) = self.limits.pop_front()?;
        Some(&self.msg[start..end])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = self.limits.len();
        (exact, Some(exact))
    }

    #[inline]
    fn count(self) -> usize {
        self.limits.len()
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        let (start, end) = self.limits.back()?;
        Some(&self.msg[*start..*end])
    }
}

impl<'m> DoubleEndedIterator for ArgsFull<'m> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let (start, end) = self.limits.pop_back()?;
        Some(&self.msg[start..end])
    }
}

impl<'m> ArgsFull<'m> {
    #[inline]
    pub fn current(&self) -> Option<&'m str> {
        let (start, end) = self.limits.front()?;
        Some(&self.msg[*start..*end])
    }
}

#[derive(Debug)]
pub enum ArgError<E> {
    Eos,
    Parse(E),
}

impl<E> From<E> for ArgError<E> {
    fn from(e: E) -> Self {
        Self::Parse(e)
    }
}

impl<E: fmt::Display> fmt::Display for ArgError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eos => f.write_str("end of string"),
            Self::Parse(e) => write!(f, "{}", e),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> Error for ArgError<E> {}
