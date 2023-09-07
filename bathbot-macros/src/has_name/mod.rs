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
            let msg = "`HasName` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, msg));
        }
        Data::Union(u) => {
            let msg = "`HasName` can only be derived for structs";

            return Err(Error::new(u.union_token.span, msg));
        }
    };

    let Fields::Named(fields) = data.fields else {
        let message = "Deriving `HasName` requires named fields";

        return Err(Error::new_spanned(ident, message));
    };

    let valid_name_field = fields.named.iter().any(|field| {
        if !matches!(field.ident, Some(ref ident) if ident == "name") {
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

    if !valid_name_field {
        let message = "Deriving `HasName` requires a field `name` \
            of type `Option<String>` or `Option<Cow<'_, str>>`";

        return Err(Error::new_spanned(ident, message));
    }

    let valid_discord_field = fields.named.iter().any(|field| match field.ident {
        Some(ref ident) if ident == "discord" => field.ty == parse_quote!(Option<Id<UserMarker>>),
        _ => false,
    });

    if !valid_discord_field {
        let message = "Deriving `HasName` requires a field `discord` of type `Id<UserMarker>`";

        return Err(Error::new_spanned(ident, message));
    }

    let path = quote!(crate::commands::osu::);

    let tokens = quote! {
        impl #generics #path HasName for #ident #generics {
            fn user_id<'ctx>(&self, ctx: &'ctx crate::core::Context) -> #path UserIdResult<'ctx> {
                if let Some(name) = self.name.as_deref() {
                    #path UserIdResult::Id(rosu_v2::request::UserId::Name(name.into()))
                } else if let Some(id) = self.discord {
                    let fut = async move {
                        match ctx.user_config().osu_id(id).await {
                            Ok(Some(user_id)) => #path UserIdFutureResult::Id(rosu_v2::request::UserId::Id(user_id)),
                            Ok(None) => #path UserIdFutureResult::NotLinked(id),
                            Err(err) => #path UserIdFutureResult::Err(err),
                        }
                    };

                    #path UserIdResult::Future(Box::pin(fut))
                } else {
                    #path UserIdResult::None
                }
            }
        }
    };

    Ok(tokens)
}
