use proc_macro2::Span;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_quote, Attribute, Block, Error, FnArg, Ident, PatType, Result, ReturnType, Token,
    Visibility,
};

use crate::util::PunctuatedExt;

pub struct CommandFun {
    pub name: Ident,
    pub ctx_arg: PatType,
    pub cmd_arg: PatType,
    pub ret: ReturnType,
    pub body: Block,
}

impl Parse for CommandFun {
    fn parse(input: ParseStream) -> Result<Self> {
        // #[...]
        input.call(Attribute::parse_outer)?;

        // pub / nothing
        input.parse::<Visibility>()?;

        // async fn
        input.parse::<Token![async]>()?;
        input.parse::<Token![fn]>()?;

        // name
        let name = input.parse::<Ident>()?;

        // ( ... )
        let content;
        parenthesized!(content in input);

        // args
        let args = Vec::<FnArg>::parse_terminated::<Token![,]>(&content)?;
        let CommandArgs { ctx, cmd } = validate_args(args)?;

        // -> ...
        let ret = input.parse::<ReturnType>()?;
        validate_return_type(&ret)?;

        // { ... }
        let body = input.parse::<Block>()?;

        Ok(Self {
            name,
            ctx_arg: ctx,
            cmd_arg: cmd,
            ret,
            body,
        })
    }
}

struct CommandArgs {
    ctx: PatType,
    cmd: PatType,
}

fn validate_args(args: Vec<FnArg>) -> Result<CommandArgs> {
    let mut ctx = None;
    let mut cmd = None;

    let ctx_check = parse_quote!(Arc<Context>);
    let cmd_check = parse_quote!(InteractionCommand);

    for arg in args {
        let pat = match arg {
            FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(receiver, "unexpected receiver argument"));
            }
            FnArg::Typed(pat) => pat,
        };

        if pat.ty == ctx_check {
            ctx = Some(pat);
        } else if pat.ty == cmd_check {
            cmd = Some(pat);
        } else {
            return Err(Error::new_spanned(
                pat,
                "args must have type `Arc<Context>` or `InteractionCommand`",
            ));
        }
    }

    Ok(CommandArgs {
        ctx: ctx.ok_or_else(|| {
            Error::new(Span::call_site(), "require argument of type `Arc<Context>`")
        })?,
        cmd: cmd.ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "require argument of type `InteractionCommand`",
            )
        })?,
    })
}

fn validate_return_type(ret: &ReturnType) -> Result<()> {
    if ret == &parse_quote!(-> Result<()>) {
        Ok(())
    } else {
        Err(Error::new_spanned(
            ret,
            "expected return type `eyre::Result<()>`",
        ))
    }
}
