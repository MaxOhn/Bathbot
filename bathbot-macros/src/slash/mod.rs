use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Error, Result, Visibility};

use crate::slash::attrs::CommandAttrs;

mod attrs;

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    match input.vis {
        Visibility::Public(_) => {}
        _ => return Err(Error::new(input.ident.span(), "type must be pub")),
    }

    let CommandAttrs {
        bucket,
        flags,
        name_lit,
    } = CommandAttrs::parse_attrs(&input.attrs)?;

    let name = input.ident;
    let name_str = name.to_string();
    let static_name = format_ident!("{}", name_str.to_uppercase(), span = name.span());
    let slash_cmd = format_ident!("slash_{}", name_str.to_lowercase(), span = name.span());
    let exec = format_ident!("{slash_cmd}__", span = name.span());
    let path = quote!(crate::core::commands::interaction::SlashCommand);

    let tokens = quote! {
        #[linkme::distributed_slice(crate::core::commands::interaction::__SLASH_COMMANDS)]
        pub static #static_name: #path = #path {
            bucket: #bucket,
            create: #name::create_command,
            exec: #exec,
            flags: #flags,
            name: #name_lit,
            id: std::sync::OnceLock::new(),
        };

        fn #exec(
            command: crate::util::interaction::InteractionCommand,
        ) -> crate::core::commands::interaction::CommandResult {
            Box::pin(#slash_cmd(command))
        }
    };

    Ok(tokens)
}
