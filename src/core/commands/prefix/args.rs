use super::stream::Stream;

pub struct Args<'m> {
    msg: &'m str,
    stream: Stream<'m>,
    pub num: Option<u64>,
}

impl<'m> Iterator for Args<'m> {
    type Item = &'m str;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, end) = self.lex()?;

        self.msg.get(start..end)
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
    pub fn new(msg: &'m str, stream: Stream<'m>, num: Option<u64>) -> Self {
        Self { msg, stream, num }
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
}
