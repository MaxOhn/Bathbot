use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Ident, Result, Token,
};

use crate::util::PunctuatedExt;

pub struct Flags {
    list: Box<[Ident]>,
}

impl Flags {
    pub fn new() -> Self {
        Self { list: Box::new([]) }
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
        Vec::parse_separated_nonempty::<Token![,]>(input)
            .map(Vec::into_boxed_slice)
            .map(|list| Self { list })
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
