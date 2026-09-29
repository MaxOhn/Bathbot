use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Ident, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

#[derive(Default)]
pub struct Flags {
    list: Box<[Ident]>,
}

impl Parse for Flags {
    fn parse(input: ParseStream) -> Result<Self> {
        let list = Punctuated::<Ident, Token![,]>::parse_separated_nonempty(input)?
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Ok(Self { list })
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
