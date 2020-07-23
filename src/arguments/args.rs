use std::{borrow::Cow, fmt, marker::PhantomData, str::FromStr};
use uwl::Stream;

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

impl<E: fmt::Debug + fmt::Display> std::error::Error for ArgError<E> {}

type Result<T, E> = ::std::result::Result<T, ArgError<E>>;

#[derive(Debug, Clone, Copy)]
struct Token {
    start: usize,
    end: usize,
}

impl Token {
    #[inline]
    fn new(start: usize, end: usize) -> Self {
        Token { start, end }
    }
    fn span(&self) -> (usize, usize) {
        (self.start, self.end)
    }
}

fn lex(stream: &mut Stream<'_>) -> Option<Token> {
    let start = stream.offset();
    if stream.current()? == b'"' {
        stream.next();
        stream.take_until(|b| b == b'"');
        let is_quote = stream.current().map_or(false, |b| b == b'"');
        stream.next();
        let end = stream.offset();
        stream.take_while_char(|c| c.is_whitespace());
        return Some(if is_quote {
            Token::new(start, end)
        } else {
            Token::new(start, stream.len())
        });
    }
    stream.take_while_char(|c| !c.is_whitespace());
    let end = stream.offset();
    stream.take_while_char(|c| c.is_whitespace());
    Some(Token::new(start, end))
}

fn remove_quotes(s: &str) -> &str {
    if s.starts_with('"') && s.ends_with('"') {
        return &s[1..s.len() - 1];
    }
    s
}

#[derive(Clone, Debug)]
pub struct Args {
    msg: String,
    args: Vec<Token>,
    offset: usize,
}

impl Args {
    pub fn new(msg: String) -> Self {
        let mut args = Vec::new();
        let mut stream = Stream::new(&msg);
        while let Some(token) = lex(&mut stream) {
            args.push(token);
        }
        Args {
            args,
            msg,
            offset: 0,
        }
    }

    #[inline]
    fn slice(&self) -> &str {
        let (start, end) = self.args[self.offset].span();
        &self.msg[start..end]
    }

    /// Move to the next argument.
    /// This increments the offset pointer.
    ///
    /// Does nothing if the message is empty.
    pub fn advance(&mut self) -> &mut Self {
        if self.is_empty() {
            return self;
        }
        self.offset += 1;
        self
    }

    /// Retrieve the current argument.
    #[inline]
    pub fn current(&self) -> Option<&str> {
        if self.is_empty() {
            return None;
        }
        Some(remove_quotes(self.slice().trim()))
    }

    /// Parse the current argument.
    #[inline]
    pub fn parse<T: FromStr>(&self) -> Result<T, T::Err> {
        T::from_str(self.current().ok_or(ArgError::Eos)?).map_err(ArgError::Parse)
    }

    /// Parse the current argument and advance.
    ///
    /// Shorthand for calling [`parse`], storing the result,
    /// calling [`next`] and returning the result.
    #[inline]
    pub fn single<T: FromStr>(&mut self) -> Result<T, T::Err> {
        let p = self.parse::<T>()?;
        self.advance();
        Ok(p)
    }

    /// Return an iterator over all unmodified arguments.
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            tokens: &self.args,
            msg: &self.msg,
        }
    }

    /// Get the original, unmodified message passed to the command.
    #[inline]
    pub fn msg(&self) -> &str {
        &self.msg
    }

    /// Return the full amount of recognised arguments.
    /// The length of the "arguments queue".
    #[inline]
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Assert that there are no more arguments left.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offset >= self.len()
    }
}

/// Access to all of the arguments, as an iterator.
#[derive(Debug)]
pub struct Iter<'a> {
    msg: &'a str,
    tokens: &'a [Token],
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (start, end) = self.tokens.get(0)?.span();
        self.tokens = &self.tokens[1..];
        Some(remove_quotes(&self.msg[start..end]))
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let last = self.tokens.len() - 1;
        let (start, end) = self.tokens.get(last)?.span();
        self.tokens = &self.tokens[..last - 1];
        Some(remove_quotes(&self.msg[start..end]))
    }
}
