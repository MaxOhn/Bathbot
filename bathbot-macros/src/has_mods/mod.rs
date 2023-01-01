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
            let message = "`HasMods` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, message));
        }
        Data::Union(u) => {
            let message = "`HasMods` can only be derived for structs";

            return Err(Error::new(u.union_token.span, message));
        }
    };

    let fields = match data.fields {
        Fields::Named(n) => n,
        _ => {
            let message = "Deriving `HasMods` requires named fields";

            return Err(Error::new(ident.span(), message));
        }
    };

    let valid_mods_field = fields.named.iter().any(|field| {
        if !matches!(field.ident, Some(ref ident) if ident == "mods") {
            return false;
        }

        let path = match field.ty {
            Type::Path(ref path) => path,
            _ => return false,
        };

        let segment = match path.path.segments.last() {
            Some(segment) if segment.ident == "Option" => segment,
            _ => return false,
        };

        let args = match segment.arguments {
            PathArguments::AngleBracketed(ref args) => args,
            _ => return false,
        };

        let path = match args.args.first() {
            Some(GenericArgument::Type(Type::Path(path))) => path,
            _ => return false,
        };

        matches!(path.path.segments.first(), Some(seg) if seg.ident == "String" || seg.ident == "Cow")
    });

    if !valid_mods_field {
        let message = "Deriving `HasMods` requires a field `mods` \
            of type `Option<String>` or `Option<Cow<'_, str>>`";

        return Err(Error::new(ident.span(), message));
    }

    let result = quote!(crate::commands::osu::ModsResult);

    let tokens = quote! {
        impl #generics crate::commands::osu::HasMods for #ident #generics {
            fn mods(&self) -> #result {
                let mods = match self.mods.as_deref() {
                    Some(mods) => mods,
                    None => return #result ::None,
                };

                if let Ok(mods) = mods.parse() {
                    return #result ::Mods(crate::util::osu::ModSelection::Exact(mods));
                }

                match crate::util::matcher::get_mods(mods) {
                    Some(mods) => #result ::Mods(mods),
                    None => #result ::Invalid
                }
            }
        }
    };

    Ok(tokens)
}
