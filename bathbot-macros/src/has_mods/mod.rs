use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result};

use crate::util::is_option_string_or_cow;

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = input;

    let data = match data {
        Data::Struct(s) => s,
        Data::Enum(e) => {
            let msg = "`HasMods` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, msg));
        }
        Data::Union(u) => {
            let msg = "`HasMods` can only be derived for structs";

            return Err(Error::new(u.union_token.span, msg));
        }
    };

    let Fields::Named(fields) = data.fields else {
        let message = "Deriving `HasMods` requires named fields";

        return Err(Error::new_spanned(ident, message));
    };

    let valid_mods_field = fields.named.iter().any(|field| {
        field.ident.as_ref().is_some_and(|ident| ident == "mods")
            && is_option_string_or_cow(&field.ty)
    });

    if !valid_mods_field {
        let message = "Deriving `HasMods` requires a field `mods` \
            of type `Option<String>` or `Option<Cow<'_, str>>`";

        return Err(Error::new_spanned(ident, message));
    }

    let tokens = quote! {
        impl #generics crate::commands::osu::HasMods for #ident #generics {
            fn mods(&self) -> crate::commands::osu::ModsResult {
                bathbot_util::osu::ModSelection::parse(self.mods.as_deref())
            }
        }
    };

    Ok(tokens)
}
