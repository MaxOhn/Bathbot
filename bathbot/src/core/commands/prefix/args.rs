use nom::{
    branch::alt,
    bytes::complete as by,
    character::complete as ch,
    combinator::{iterator, map_opt, ParserIterator},
    error::Error as NomError,
    sequence::{delimited, terminated},
    Err as NomErr, IResult,
};

type ItemError<'m> = NomError<&'m str>;
type ItemFn<'m> = fn(&'m str) -> IResult<&'m str, &'m str, ItemError<'m>>;

pub struct Args<'m> {
    iter: ParserIterator<&'m str, ItemError<'m>, ItemFn<'m>>,
    pub num: Option<u64>,
}

impl<'m> Args<'m> {
    pub fn new(content: &'m str, num: Option<u64>) -> Self {
        Self {
            iter: iterator(content, Self::next_item),
            num,
        }
    }

    pub fn rest(self) -> &'m str {
        match self.iter.finish() {
            Ok((rest, _)) => rest,
            Err(err) => {
                error!("Error while getting rest of args: {err}");

                match err {
                    NomErr::Incomplete(_) => "",
                    NomErr::Error(err) | NomErr::Failure(err) => err.input,
                }
            }
        }
    }

    fn next_item(input: &'m str) -> IResult<&'m str, &'m str, ItemError<'m>> {
        let quoted = delimited(ch::char('"'), by::take_until1("\""), ch::char('"'));

        let simple = map_opt(by::take_till(char::is_whitespace), |item: &str| {
            (!item.is_empty()).then_some(item)
        });

        terminated(alt((quoted, simple)), ch::space0)(input)
    }
}

impl<'m> Iterator for Args<'m> {
    type Item = &'m str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (&mut self.iter).next()
    }
}
