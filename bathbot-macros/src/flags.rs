use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Ident, Result, Token,
};

pub struct Flags {
    tokens: TokenStream,
}

impl Flags {
    pub fn new(bits: u8) -> Self {
        Self {
            tokens: quote!(#bits),
        }
    }
}

pub fn parse_flags(attrs: &[Attribute]) -> Result<Flags> {
    match attrs.iter().find(|attr| attr.path().is_ident("flags")) {
        Some(attr) => attr.parse_args(),
        None => Ok(Flags::new(0)),
    }
}

// TODO: cleanup
impl Parse for Flags {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut tokens = quote!(0);

        loop {
            let flag = input.step(|cursor| {
                let message = if let Some((ident, rest)) = cursor.ident() {
                    if accept_as_flag(&ident) {
                        return Ok((ident, rest));
                    }

                    r#"expected "AUTHORITY", "EPHEMERAL", "ONLY_GUILDS", "ONLY_OWNER", or "SKIP_DEFER""#
                } else {
                    "expected identifier"
                };

                Err(cursor.error(message))
            })?;

            tokens.extend(quote!( + crate::core::commands::CommandFlags::#flag.bits()));

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(Self { tokens })
    }
}

impl ToTokens for Flags {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let bits = &self.tokens;

        tokens.extend(
            quote!(unsafe { crate::core::commands::CommandFlags::from_bits_unchecked(#bits) }),
        );
    }
}

fn accept_as_flag(ident: &Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "AUTHORITY" | "EPHEMERAL" | "ONLY_GUILDS" | "ONLY_OWNER" | "SKIP_DEFER"
    )
}
