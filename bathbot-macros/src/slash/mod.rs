use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Error, Result, Visibility};

use crate::{bucket::parse_bucket, flags::parse_flags};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    match input.vis {
        Visibility::Public(_) => {}
        _ => return Err(Error::new(input.ident.span(), "type must be pub")),
    }

    let name = input.ident;
    let name_str = name.to_string();
    let static_name = format_ident!("{}_SLASH", name_str.to_uppercase());
    let slash_cmd = format_ident!("slash_{}", name_str.to_lowercase());
    let exec = format_ident!("{slash_cmd}__");
    let bucket = parse_bucket(&input.attrs)?;
    let flags = parse_flags(&input.attrs)?.into_tokens();
    let path = quote!(crate::core::commands::slash::SlashCommand);

    let tokens = quote! {
        pub static #static_name: #path = #path {
            bucket: #bucket,
            create: #name::create_command,
            exec: #exec,
            flags: #flags,
        };

        pub fn #exec(
            ctx: std::sync::Arc<crate::core::Context>,
            command: crate::util::interaction::InteractionCommand,
        ) -> crate::core::commands::slash::CommandResult {
            Box::pin(#slash_cmd(ctx, command))
        }
    };

    Ok(tokens)
}
