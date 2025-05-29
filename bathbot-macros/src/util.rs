use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{
    Ident, LitStr, Path, Result, Token,
    parse::{Parse, ParseStream},
    token::Token as TokenTrait,
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

pub struct AsOption<T>(pub Option<T>);

impl<T: ToTokens> ToTokens for AsOption<T> {
    fn to_tokens(&self, stream: &mut TokenStream2) {
        match &self.0 {
            Some(o) => stream.extend(quote!(Some(#o))),
            None => stream.extend(quote!(None)),
        }
    }
}

pub enum LitOrConst {
    Lit(LitStr),
    Const(Path),
}

impl Parse for LitOrConst {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(LitStr) {
            input.parse().map(Self::Lit)
        } else if lookahead.peek(Ident) || lookahead.peek(Token![::]) {
            input.parse().map(Self::Const)
        } else {
            Err(lookahead.error())
        }
    }
}

impl ToTokens for LitOrConst {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            LitOrConst::Lit(lit) => lit.to_tokens(tokens),
            LitOrConst::Const(path) => tokens.extend(quote! {
                {
                    const _: &str = #path;

                    #path
                }
            }),
        }
    }
}
