use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Result;

pub use self::{attrs::CommandAttrs, command::CommandFun};

mod attrs;
mod command;

pub fn impl_cmd(attrs: CommandAttrs, fun: CommandFun) -> Result<TokenStream> {
    let CommandAttrs {
        name: attr_name,
        dm_permission,
        flags,
    } = attrs;

    let CommandFun {
        name: cmd_name,
        ctx_arg,
        cmd_arg,
        ret,
        body,
    } = fun;

    let cmd_name_str = cmd_name.to_string();

    let static_name = format_ident!(
        "{}_MSG",
        cmd_name_str.to_uppercase(),
        span = cmd_name.span()
    );

    let create = format_ident!("create_{cmd_name}__");
    let exec = format_ident!("exec_{cmd_name}__");

    let path = quote!(crate::core::commands::interaction::MessageCommand);

    let tokens = quote! {
        #[linkme::distributed_slice(crate::core::commands::interaction::__MSG_COMMANDS)]
        pub static #static_name: #path = #path {
            create: #create,
            exec: #exec,
            flags: #flags,
            name: #attr_name,
            id: std::sync::OnceLock::new(),
        };

        fn #create() -> crate::core::commands::interaction::twilight_command::Command {
            crate::core::commands::interaction::twilight_command::Command {
                application_id: None,
                default_member_permissions: None,
                description: String::new(),
                description_localizations: None,
                dm_permission: #dm_permission,
                guild_id: None,
                id: None,
                kind: ::twilight_model::application::command::CommandType::Message,
                name: #attr_name.to_owned(),
                name_localizations: None,
                nsfw: None,
                options: Vec::new(),
                version: ::twilight_model::id::Id::new(1),
            }
        }

        fn #exec(
            ctx: std::sync::Arc<crate::core::Context>,
            command: crate::util::interaction::InteractionCommand,
        ) -> crate::core::commands::interaction::CommandResult {
            Box::pin(#cmd_name(ctx, command))
        }

        async fn #cmd_name(#ctx_arg, #cmd_arg) #ret {
            #body
        }
    };

    Ok(tokens)
}
