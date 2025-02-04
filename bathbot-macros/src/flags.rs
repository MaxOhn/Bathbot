use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    Ident, Result, Token,
};

use crate::util::PunctuatedExt;

#[derive(Default)]
pub struct Flags {
    list: Box<[Ident]>,
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

        tokens.extend(quote!(crate::core::commands::CommandFlags::from_bits_retain(#sum)));
    }
}
