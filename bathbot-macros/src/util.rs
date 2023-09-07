use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    token::Token as TokenTrait,
    Ident, Lit, Result,
};

/// Revamping utility from [`Punctuated`] for the purpose of storing items
/// more efficiently.
///
/// [`Punctuated`]: syn::punctuated::Punctuated
pub trait PunctuatedExt<T> {
    /// Parses one or more occurrences of `T` separated by punctuation of type
    /// `P`, not accepting trailing punctuation.
    ///
    /// Parsing continues as long as punctuation `P` is present at the head of
    /// the stream. This method returns upon parsing a `T` and observing that it
    /// is not followed by a `P`, even if there are remaining tokens in the
    /// stream.
    fn parse_separated_nonempty<P: Parse + TokenTrait>(input: ParseStream) -> Result<Vec<T>>
    where
        T: Parse,
    {
        Self::parse_separated_nonempty_with::<P>(input, T::parse)
    }

    /// Parses one or more occurrences of `T` using the given parse function,
    /// separated by punctuation of type `P`, not accepting trailing
    /// punctuation.
    ///
    /// Like [`parse_separated_nonempty`], may complete early without parsing
    /// the entire content of this stream.
    fn parse_separated_nonempty_with<P: Parse + TokenTrait>(
        input: ParseStream,
        parser: fn(ParseStream) -> Result<T>,
    ) -> Result<Vec<T>>;

    /// Parses zero or more occurrences of `T` separated by punctuation of type
    /// `P`, with optional trailing punctuation.
    ///
    /// Parsing continues until the end of this parse stream. The entire content
    /// of this parse stream must consist of `T` and `P`.
    fn parse_terminated<P: Parse>(input: ParseStream) -> Result<Vec<T>>
    where
        T: Parse,
    {
        Self::parse_terminated_with::<P>(input, T::parse)
    }

    /// Parses zero or more occurrences of `T` using the given parse function,
    /// separated by punctuation of type `P`, with optional trailing
    /// punctuation.
    ///
    /// Like [`parse_terminated`], the entire content of this stream is expected
    /// to be parsed.
    fn parse_terminated_with<P: Parse>(
        input: ParseStream,
        parser: fn(ParseStream) -> Result<T>,
    ) -> Result<Vec<T>>;
}

impl<T> PunctuatedExt<T> for Vec<T> {
    fn parse_separated_nonempty_with<P: Parse + TokenTrait>(
        input: ParseStream,
        parser: fn(ParseStream) -> Result<T>,
    ) -> Result<Self> {
        let mut vec = Vec::new();

        loop {
            vec.push(parser(input)?);

            if !P::peek(input.cursor()) {
                break;
            }

            input.parse::<P>()?;
        }

        Ok(vec)
    }

    fn parse_terminated_with<P: Parse>(
        input: ParseStream,
        parser: fn(ParseStream) -> Result<T>,
    ) -> Result<Self> {
        let mut vec = Vec::new();

        loop {
            if input.is_empty() {
                break;
            }

            vec.push(parser(input)?);

            if input.is_empty() {
                break;
            }

            input.parse::<P>()?;
        }

        Ok(vec)
    }
}

pub trait LitExt {
    fn to_string(&self) -> String;
    fn to_ident(&self) -> Ident;
}

impl LitExt for Lit {
    fn to_string(&self) -> String {
        match self {
            Lit::Str(s) => s.value(),
            Lit::ByteStr(s) => unsafe { String::from_utf8_unchecked(s.value()) },
            Lit::Char(c) => c.value().to_string(),
            Lit::Byte(b) => (b.value() as char).to_string(),
            _ => panic!("values must be a (byte)string or a char"),
        }
    }

    fn to_ident(&self) -> Ident {
        Ident::new(&self.to_string(), self.span())
    }
}

pub trait IdentExt: Sized {
    fn to_uppercase(&self) -> Self;
}

impl IdentExt for Ident {
    fn to_uppercase(&self) -> Self {
        format_ident!("{}", self.to_string().to_uppercase(), span = self.span())
    }
}

pub struct AsOption<T>(pub Option<T>);

impl<T: ToTokens> ToTokens for AsOption<T> {
    fn to_tokens(&self, stream: &mut TokenStream2) {
        match &self.0 {
            Some(o) => stream.extend(quote!(Some(#o))),
            None => stream.extend(quote!(None)),
        }
    }
}
