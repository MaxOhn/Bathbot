use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{parse_quote, spanned::Spanned, Error, FnArg, Result};

pub use self::command::CommandFun;
use self::{
    command::Argument,
    options::{parse_options, Options},
};

mod command;
mod options;

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

    let Options {
        aliases,
        desc,
        help,
        usage,
        examples,
        bucket,
        flags,
        group,
    } = parse_options(&fun.attrs)?;

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

    let flags = flags.into_tokens();
    let path = quote!(crate::core::commands::prefix::PrefixCommand);

    let tokens = quote! {
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
            ctx: Arc<Context>,
            msg: &'fut twilight_model::channel::Message,
            args: crate::core::commands::prefix::Args<'fut>,
            permissions: Option<::twilight_model::guild::Permissions>,
        ) -> crate::core::commands::prefix::CommandResult<'fut> {
            Box::pin(#fun_name(ctx, msg, args, permissions))
        }
    };

    Ok(tokens)
}

fn validate_args(fun: &mut CommandFun) -> Result<()> {
    let mut iter = fun.args.iter_mut();

    match iter.next() {
        Some(arg) if arg.ty == parse_quote! { Arc<Context> } => {}
        Some(arg) => {
            return Err(Error::new(
                arg.ty.span(),
                "expected first argument to be of type `Arc<Context>`",
            ));
        }
        None => {
            return Err(Error::new(
                fun.name.span(),
                "expected first argument to be of type `Arc<Context>`",
            ));
        }
    }

    match iter.next() {
        Some(arg) if arg.ty == parse_quote! { &Message } => {
            arg.ty = parse_quote! { &'fut twilight_model::channel::Message };
        }
        Some(arg) => {
            return Err(Error::new(
                arg.ty.span(),
                "expected second argument to be of type `&Message`",
            ));
        }
        None => {
            return Err(Error::new(
                fun.name.span(),
                "expected second argument to be of type `&Message`",
            ));
        }
    }

    let (args, permissions, swap) = match (iter.next(), iter.next()) {
        (None, None) => {
            let args: FnArg = parse_quote!(_: crate::core::commands::prefix::Args<'fut>);
            let args = Argument::try_from(args)?;
            let permissions: FnArg =
                parse_quote!(_: Option<::twilight_model::guild::Permissions>);
            let permissions = Argument::try_from(permissions)?;

            (Some(args), Some(permissions), false)
        }
        (Some(arg), None) if arg.ty == parse_quote! { Args<'_> } => {
            arg.ty = parse_quote! { crate::core::commands::prefix::Args<'fut> };

            let permissions: FnArg =
                parse_quote!(_: Option<::twilight_model::guild::Permissions>);

            (None, Some(Argument::try_from(permissions)?), false)
        }
        (Some(arg), None) if arg.ty == parse_quote! { Option<Permissions> } => {
            let args: FnArg = parse_quote!(_: crate::core::commands::prefix::Args<'fut>);

            (Some(Argument::try_from(args)?), None, true)
        }
        (Some(arg), None) => {
            return Err(Error::new(
                arg.ty.span(),
                "expected third argument to be of type `Args<'_>` or `Option<Permissions>`",
            ))
        }
        (Some(arg1), Some(arg2)) if arg1.ty == parse_quote! { Args<'_> } && arg2.ty == parse_quote! { Option<Permissions> } => {
            arg1.ty = parse_quote! { crate::core::commands::prefix::Args<'fut> };

            (None, None, false)
        },
        (Some(arg1), Some(arg2)) if arg2.ty == parse_quote! { Args<'_> } && arg1.ty == parse_quote! { Option<Permissions> } => {
            arg2.ty = parse_quote! { crate::core::commands::prefix::Args<'fut> };

            (None, None, true)
        },
        (Some(arg), Some(_)) => {
            return Err(Error::new(
                arg.ty.span(),
                "expected third and fourth arguments to be of type `Args<'_>` and `Option<Permissions>`",
            ))
        }
        (None, Some(_)) => unreachable!(),
    };

    if iter.count() > 0 {
        return Err(Error::new(
            fun.name.span(),
            "expected at most four arguments",
        ));
    }

    if let Some(args) = args {
        fun.args.push(args);
    }

    if let Some(permissions) = permissions {
        fun.args.push(permissions);
    }

    if swap {
        fun.args.swap(2, 3);
    }

    Ok(())
}
