use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result};

const VALID_FIELDS: [&str; 10] = [
    "author",
    "color",
    "description",
    "fields",
    "footer",
    "image",
    "timestamp",
    "title",
    "thumbnail",
    "url",
];

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let DeriveInput { ident, data, .. } = input;

    let data = match data {
        Data::Struct(s) => s,
        Data::Enum(e) => {
            let message = "`EmbedData` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, message));
        }
        Data::Union(u) => {
            let message = "`EmbedData` can only be derived for structs";

            return Err(Error::new(u.union_token.span, message));
        }
    };

    let named_fields = match data.fields {
        Fields::Named(n) => n.named,
        _ => {
            let message = "Deriving `EmbedData` requires named fields";

            return Err(Error::new(ident.span(), message));
        }
    };

    let mut calls = TokenStream::new();

    for field in named_fields {
        let field_ident = match field.ident {
            Some(ident) => ident,
            None => {
                let message = "Deriving `EmbedData` requires named fields";

                return Err(Error::new(ident.span(), message));
            }
        };

        let ident_str = field_ident.to_string();

        if VALID_FIELDS.contains(&ident_str.as_str()) {
            calls.extend(quote!(.#field_ident(self.#field_ident)));
        } else {
            let message = format!(
                "Invalid field name for `EmbedData`, must be one of: {}",
                VALID_FIELDS.join(", ")
            );

            return Err(Error::new(field_ident.span(), message));
        }
    }

    let tokens = quote! {
        impl crate::embeds::EmbedData for #ident {
            fn build(self) -> ::bathbot_util::EmbedBuilder {
                bathbot_util::EmbedBuilder::new()
                    #calls
            }
        }
    };

    Ok(tokens)
}
