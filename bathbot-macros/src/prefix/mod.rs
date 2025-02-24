use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Error, Result, parse_quote};

use self::attrs::CommandAttrs;
pub use self::command::CommandFun;

mod attrs;
mod command;

pub fn attr(tokens: TokenStream) -> Result<()> {
    if !tokens.is_empty() {
        let message = format!("expected `#[command]`, got #[command({tokens})]");

        Err(Error::new(Span::call_site(), message))
    } else {
        Ok(())
    }
}

pub fn fun(mut fun: CommandFun) -> Result<TokenStream2> {
    let name_str = fun.name.to_string();

    if !name_str.starts_with("prefix_") {
        let message = "function name must start with `prefix_`";

        return Err(Error::new(fun.name.span(), message));
    }

    validate_args(&mut fun)?;

    let fun_name = &fun.name;
    let name_str = name_str[7..].to_owned();
    let static_name = format_ident!("{}_PREFIX", name_str.to_uppercase());
    let exec = format_ident!("{name_str}__");

    let CommandAttrs {
        aliases,
        desc,
        help,
        usage,
        examples,
        bucket,
        flags,
        group,
    } = CommandAttrs::parse_attrs(&fun.attrs)?;

    let desc = match desc {
        Some(desc) => desc,
        None => {
            return Err(Error::new(
                Span::call_site(),
                r#"must specify #[desc("...")]"#,
            ));
        }
    };

    let group = match group {
        Some(ident) => ident,
        None => {
            return Err(Error::new(
                Span::call_site(),
                r#"must specify #[group(...)]"#,
            ));
        }
    };

    let path = quote!(crate::core::commands::prefix::PrefixCommand);

    let tokens = quote! {
        #[linkme::distributed_slice(crate::core::commands::prefix::__PREFIX_COMMANDS)]
        pub static #static_name: #path = #path {
            names: &[#name_str, #(#aliases),*],
            desc: #desc,
            help: #help,
            usage: #usage,
            examples: &[#(#examples),*],
            bucket: #bucket,
            flags: #flags,
            group: crate::core::commands::prefix::PrefixCommandGroup::#group,
            exec: #exec,
        };

        #fun

        fn #exec<'fut>(
            msg: &'fut twilight_model::channel::Message,
            args: crate::core::commands::prefix::Args<'fut>,
            permissions: Option<::twilight_model::guild::Permissions>,
        ) -> crate::core::commands::prefix::CommandResult<'fut> {
            Box::pin(#fun_name(msg, args, permissions))
        }
    };

    Ok(tokens)
}

fn validate_args(fun: &mut CommandFun) -> Result<()> {
    let mut iter = fun.args.iter_mut();

    match iter.next() {
        Some(arg) if arg.ty == parse_quote! { &Message } => {
            arg.ty = parse_quote! { &'fut twilight_model::channel::Message };
        }
        Some(arg) => {
            return Err(Error::new_spanned(
                &arg.ty,
                "expected first argument to be of type `&Message`",
            ));
        }
        None => {
            return Err(Error::new_spanned(
                &fun.name,
                "expected first argument to be of type `&Message`",
            ));
        }
    }

    let (args, permissions, swap) = match (iter.next(), iter.next()) {
        (None, None) => {
            let args = parse_quote!(_: crate::core::commands::prefix::Args<'fut>);
            let permissions = parse_quote!(_: Option<::twilight_model::guild::Permissions>);

            (Some(args), Some(permissions), false)
        }
        (Some(arg), None) if arg.ty == parse_quote! { Args<'_> } => {
            arg.ty = parse_quote! { crate::core::commands::prefix::Args<'fut> };

            let permissions = parse_quote!(_: Option<::twilight_model::guild::Permissions>);

            (None, Some(permissions), false)
        }
        (Some(arg), None) if arg.ty == parse_quote! { Option<Permissions> } => {
            let args = parse_quote!(_: crate::core::commands::prefix::Args<'fut>);

            (Some(args), None, true)
        }
        (Some(arg), None) => {
            return Err(Error::new_spanned(
                &arg.ty,
                "expected second argument to be of type `Args<'_>` or `Option<Permissions>`",
            ));
        }
        (Some(arg1), Some(arg2))
            if arg1.ty == parse_quote! { Args<'_> }
                && arg2.ty == parse_quote! { Option<Permissions> } =>
        {
            arg1.ty = parse_quote! { crate::core::commands::prefix::Args<'fut> };

            (None, None, false)
        }
        (Some(arg1), Some(arg2))
            if arg2.ty == parse_quote! { Args<'_> }
                && arg1.ty == parse_quote! { Option<Permissions> } =>
        {
            arg2.ty = parse_quote! { crate::core::commands::prefix::Args<'fut> };

            (None, None, true)
        }
        (Some(arg), Some(_)) => {
            return Err(Error::new_spanned(
                &arg.ty,
                "expected second and third arguments to be of type `Args<'_>` and `Option<Permissions>`",
            ));
        }
        (None, Some(_)) => unreachable!(),
    };

    if iter.count() > 0 {
        return Err(Error::new_spanned(
            &fun.name,
            "expected at most three arguments",
        ));
    }

    if let Some(args) = args {
        fun.args.push(args);
    }

    if let Some(permissions) = permissions {
        fun.args.push(permissions);
    }

    if swap {
        fun.args.swap(1, 2);
    }

    Ok(())
}
