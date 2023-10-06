use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, DeriveInput, Error, LitBool, LitStr, Result, Visibility};

use crate::{bucket::parse_bucket, flags::parse_flags};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    match input.vis {
        Visibility::Public(_) => {}
        _ => return Err(Error::new(input.ident.span(), "type must be pub")),
    }

    let name = input.ident;
    let name_str = name.to_string();
    let static_name = format_ident!("{}", name_str.to_uppercase(), span = name.span());
    let slash_cmd = format_ident!("slash_{}", name_str.to_lowercase(), span = name.span());
    let exec = format_ident!("{slash_cmd}__", span = name.span());
    let bucket = parse_bucket(&input.attrs)?;
    let name_lit = parse_name(&input.attrs)?;
    let flags = parse_flags(&input.attrs)?;
    let path = quote!(crate::core::commands::interaction::SlashCommand);

    let tokens = quote! {
        #[linkme::distributed_slice(crate::core::commands::interaction::__SLASH_COMMANDS)]
        pub static #static_name: #path = #path {
            bucket: #bucket,
            create: #name::create_command,
            exec: #exec,
            flags: #flags,
            name: #name_lit,
        };

        fn #exec(
            ctx: std::sync::Arc<crate::core::Context>,
            command: crate::util::interaction::InteractionCommand,
        ) -> crate::core::commands::interaction::CommandResult {
            Box::pin(#slash_cmd(ctx, command))
        }
    };

    Ok(tokens)
}

pub fn parse_name(attrs: &[Attribute]) -> Result<LitStr> {
    match attrs.iter().find(|attr| attr.path().is_ident("command")) {
        Some(attr) => {
            let mut name = None;

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    name = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("desc") {
                    let _: LitStr = meta.value()?.parse()?;
                } else if meta.path.is_ident("help") {
                    let _: LitStr = meta.value()?.parse()?;
                } else if meta.path.is_ident("dm_permission") {
                    let _: LitBool = meta.value()?.parse()?;
                } else {
                    return Err(meta.error(
                        "`SlashCommand` expected `name`, `desc`, `help`, or `dm_permission` \
                        command meta",
                    ));
                }

                Ok(())
            })?;

            name.ok_or_else(|| Error::new_spanned(attr, "missing `name` attribute"))
        }
        None => Err(Error::new(Span::call_site(), "missing `command` attribute")),
    }
}
