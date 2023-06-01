use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
    Attribute, Ident, Result,
};

pub struct Flags {
    list: Punctuated<Ident, Comma>,
}

impl Flags {
    pub fn new() -> Self {
        Self {
            list: Punctuated::new(),
        }
    }
}

pub fn parse_flags(attrs: &[Attribute]) -> Result<Flags> {
    match attrs.iter().find(|attr| attr.path().is_ident("flags")) {
        Some(attr) => attr.parse_args(),
        None => Ok(Flags::new()),
    }
}

impl Parse for Flags {
    fn parse(input: ParseStream) -> Result<Self> {
        Punctuated::parse_separated_nonempty(input).map(|list| Self { list })
    }
}

impl ToTokens for Flags {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut flags = self.list.iter();

        let Some(flag) = flags.next() else {
            tokens.extend(quote!(crate::core::commands::CommandFlags::empty()));

            return;
        };

        let mut sum = quote!(crate::core::commands::CommandFlags:: #flag .bits());

        for bit in flags.map(|flag| quote!(+ crate::core::commands::CommandFlags:: #flag .bits())) {
            sum.extend(bit)
        }

        tokens.extend(
            quote!(unsafe { crate::core::commands::CommandFlags::from_bits_unchecked(#sum) }),
        );
    }
}
