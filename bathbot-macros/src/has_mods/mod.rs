use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, GenericArgument, PathArguments, Result, Type};

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
        if !matches!(field.ident, Some(ref ident) if ident == "mods") {
            return false;
        }

        let Type::Path(ref path) = field.ty else {
            return false;
        };

        let segment = match path.path.segments.last() {
            Some(segment) if segment.ident == "Option" => segment,
            _ => return false,
        };

        let PathArguments::AngleBracketed(ref args) = segment.arguments else {
            return false;
        };

        let Some(GenericArgument::Type(Type::Path(path))) = args.args.first() else {
            return false;
        };

        matches!(path.path.segments.first(), Some(seg) if seg.ident == "String" || seg.ident == "Cow")
    });

    if !valid_mods_field {
        let message = "Deriving `HasMods` requires a field `mods` \
            of type `Option<String>` or `Option<Cow<'_, str>>`";

        return Err(Error::new_spanned(ident, message));
    }

    let result = quote!(crate::commands::osu::ModsResult);

    let tokens = quote! {
        impl #generics crate::commands::osu::HasMods for #ident #generics {
            fn mods(&self) -> #result {
                let mods = match self.mods.as_deref() {
                    Some(mods) => mods,
                    None => return #result ::None,
                };

                if let Some(mods) = rosu_v2::model::mods::GameModsIntermode::try_from_acronyms(mods) {
                    return #result ::Mods(bathbot_util::osu::ModSelection::Exact(mods));
                }

                match bathbot_util::matcher::get_mods(mods) {
                    Some(mods) => #result ::Mods(mods),
                    None => #result ::Invalid
                }
            }
        }
    };

    Ok(tokens)
}
