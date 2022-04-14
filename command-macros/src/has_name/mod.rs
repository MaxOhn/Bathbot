use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_quote, Data, DeriveInput, Error, Fields, GenericArgument, PathArguments, Result, Type,
};

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
            let message = "`HasName` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, message));
        }
        Data::Union(u) => {
            let message = "`HasName` can only be derived for structs";

            return Err(Error::new(u.union_token.span, message));
        }
    };

    let fields = match data.fields {
        Fields::Named(n) => n,
        _ => {
            let message = "Deriving `HasName` requires named fields";

            return Err(Error::new(ident.span(), message));
        }
    };

    let valid_name_field = fields.named.iter().any(|field| {
        if !matches!(field.ident, Some(ref ident) if ident == "name") {
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

    if !valid_name_field {
        let message = "Deriving `HasName` requires a field `name` \
            of type `Option<String>` or `Option<Cow<'_, str>>`";

        return Err(Error::new(ident.span(), message));
    }

    let valid_discord_field = fields.named.iter().any(|field| match field.ident {
        Some(ref ident) if ident == "discord" => field.ty == parse_quote!(Option<Id<UserMarker>>),
        _ => false,
    });

    if !valid_discord_field {
        let message = "Deriving `HasName` requires a field `discord` of type `Id<UserMarker>`";

        return Err(Error::new(ident.span(), message));
    }

    let path = quote!(crate::commands::osu::);

    let tokens = quote! {
        impl #generics #path HasName for #ident #generics {
            fn username<'ctx>(&self, ctx: &'ctx crate::core::Context) -> #path UsernameResult<'ctx> {
                if let Some(name) = self.name.as_deref() {
                    #path UsernameResult::Name(name.into())
                } else if let Some(id) = self.discord {
                    let fut = async move {
                        match ctx.psql().get_user_osu(id).await {
                            Ok(Some(osu)) => #path UsernameFutureResult::Name(osu.into_username()),
                            Ok(None) => #path UsernameFutureResult::NotLinked(id),
                            Err(err) => #path UsernameFutureResult::Err(err),
                        }
                    };

                    #path UsernameResult::Future(Box::pin(fut))
                } else {
                    #path UsernameResult::None
                }
            }
        }
    };

    Ok(tokens)
}
