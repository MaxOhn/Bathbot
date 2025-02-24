use proc_macro2::Span;
use syn::{
    Attribute, Block, Error, FnArg, Ident, PatType, Result, ReturnType, Token, Visibility,
    parenthesized,
    parse::{Parse, ParseStream},
    parse_quote,
};

use crate::util::PunctuatedExt;

pub struct CommandFun {
    pub name: Ident,
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
        let CommandArgs { cmd } = validate_args(args)?;

        // -> ...
        let ret = input.parse::<ReturnType>()?;
        validate_return_type(&ret)?;

        // { ... }
        let body = input.parse::<Block>()?;

        Ok(Self {
            name,
            cmd_arg: cmd,
            ret,
            body,
        })
    }
}

struct CommandArgs {
    cmd: PatType,
}

fn validate_args(args: Vec<FnArg>) -> Result<CommandArgs> {
    let mut cmd = None;

    let cmd_check = parse_quote!(InteractionCommand);

    for arg in args {
        let pat = match arg {
            FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(receiver, "unexpected receiver argument"));
            }
            FnArg::Typed(pat) => pat,
        };

        if pat.ty == cmd_check {
            cmd = Some(pat);
        } else {
            return Err(Error::new_spanned(
                pat,
                "args must have type `InteractionCommand`",
            ));
        }
    }

    Ok(CommandArgs {
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
