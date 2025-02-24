use nom::{
    Err as NomErr, IResult,
    branch::alt,
    bytes::complete as by,
    character::complete as ch,
    combinator::{ParserIterator, iterator, map_opt},
    error::Error as NomError,
    sequence::{delimited, terminated},
};

type ItemError<'m> = NomError<&'m str>;
type ItemFn<'m> = fn(&'m str) -> IResult<&'m str, &'m str, ItemError<'m>>;

pub struct Args<'m> {
    iter: ParserIterator<&'m str, ItemError<'m>, ItemFn<'m>>,
    pub num: ArgsNum,
}

impl<'m> Args<'m> {
    pub fn new(content: &'m str, num: ArgsNum) -> Self {
        Self {
            iter: iterator(content, Self::next_item),
            num,
        }
    }

    pub fn rest(self) -> &'m str {
        match self.iter.finish() {
            Ok((rest, _)) => rest,
            Err(err) => {
                error!(?err, "Error while getting rest of args");

                match err {
                    NomErr::Incomplete(_) => "",
                    NomErr::Error(err) | NomErr::Failure(err) => err.input,
                }
            }
        }
    }

    fn next_item(input: &'m str) -> IResult<&'m str, &'m str, ItemError<'m>> {
        let quote_delimited = |start: char, end: char| {
            delimited(
                ch::char(start),
                by::take_till1(move |c| c == end),
                ch::char(end),
            )
        };

        let simple = map_opt(by::take_till(char::is_whitespace), |item: &str| {
            (!item.is_empty()).then_some(item)
        });

        let options = (
            quote_delimited('"', '"'),
            quote_delimited('\'', '\''),
            quote_delimited('“', '“'),
            quote_delimited('«', '»'),
            quote_delimited('„', '“'),
            quote_delimited('“', '”'),
            simple,
        );

        terminated(alt(options), ch::space0)(input)
    }
}

impl<'m> Iterator for Args<'m> {
    type Item = &'m str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (&mut self.iter).next()
    }
}

#[derive(Copy, Clone)]
pub enum ArgsNum {
    Value(u32),
    Random,
    None,
}

impl ArgsNum {
    pub fn to_string_opt(self) -> Option<String> {
        match self {
            ArgsNum::Value(n) => Some(n.to_string()),
            ArgsNum::Random => Some("?".to_string()),
            ArgsNum::None => None,
        }
    }
}
