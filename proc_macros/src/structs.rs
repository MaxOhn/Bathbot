use crate::util::{Argument, AsOption, Parenthesised};

use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{
    braced,
    parse::{Error, Parse, ParseStream, Result},
    parse_quote,
    spanned::Spanned,
    Attribute, Block, FnArg, Ident, Pat, ReturnType, Stmt, Token, Type, Visibility,
};

pub struct CommandFun {
    // #[...]
    pub attributes: Vec<Attribute>,
    // pub / nothing
    pub visibility: Visibility,
    // name
    pub name: Ident,
    // (...)
    pub args: Vec<Argument>,
    // -> ...
    pub ret: Type,
    // { ... }
    pub body: Vec<Stmt>,
}

impl Parse for CommandFun {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        // #[...]
        let attributes = input.call(Attribute::parse_outer)?;

        // pub / nothing
        let visibility = input.parse::<Visibility>()?;

        // async fn
        input.parse::<Token![async]>()?;
        input.parse::<Token![fn]>()?;

        // name
        let name = input.parse::<Ident>()?;

        // (_: Arc<Context>, _: &Message)
        let Parenthesised(args) = input.parse::<Parenthesised<FnArg>>()?;
        let args = args
            .into_iter()
            .map(parse_argument)
            .collect::<Result<Vec<_>>>()?;
        let mut iter = args.iter();
        let valid = match iter.next() {
            Some(arg) => &arg.kind == &parse_quote! { Arc<Context> },
            None => false,
        };
        if !valid {
            return Err(input.error("expected first argument of type `Arc<Context>`"));
        }
        let valid = match iter.next() {
            Some(arg) => &arg.kind == &parse_quote! { &Message },
            None => false,
        };
        if !valid {
            return Err(input.error("expected second argument of type `&Message`"));
        }

        // -> BotResult<()>
        let ret = match input.parse::<ReturnType>()? {
            ReturnType::Type(_, t) => {
                if &t == &parse_quote! { BotResult<()> } {
                    *t
                } else {
                    return Err(input.error("expected return type `BotResult<()>`"));
                }
            }
            ReturnType::Default => return Err(input.error("expected a return value")),
        };

        // { ... }
        let body_content;
        braced!(body_content in input);
        let body = body_content.call(Block::parse_within)?;

        Ok(Self {
            attributes,
            visibility,
            name,
            args,
            ret,
            body,
        })
    }
}

impl ToTokens for CommandFun {
    fn to_tokens(&self, stream: &mut TokenStream2) {
        let Self {
            attributes: _,
            visibility,
            name,
            args,
            ret,
            body,
        } = self;
        stream.extend(quote! {
            #visibility fn #name<'fut> (#(#args),*) -> futures::future::BoxFuture<'fut, #ret> {
                use futures::future::FutureExt;
                async move { #(#body)* }.boxed()
            }
        });
    }
}

fn parse_argument(arg: FnArg) -> Result<Argument> {
    match arg {
        FnArg::Typed(typed) => {
            let pat = typed.pat;
            match *pat {
                Pat::Ident(id) => {
                    let name = id.ident;
                    let mutable = id.mutability;
                    Ok(Argument {
                        mutable,
                        name,
                        kind: *typed.ty,
                    })
                }
                Pat::Wild(wild) => {
                    let token = wild.underscore_token;
                    let name = Ident::new("_", token.spans[0]);
                    Ok(Argument {
                        mutable: None,
                        name,
                        kind: *typed.ty,
                    })
                }
                _ => Err(Error::new(
                    pat.span(),
                    "expected either _ or identifier before `:`",
                )),
            }
        }
        FnArg::Receiver(_) => Err(Error::new(
            arg.span(),
            "expected arguments of the form `identifier: type`",
        )),
    }
}

#[derive(Default)]
pub struct Options {
    pub aliases: Vec<String>,
    pub short_desc: Option<String>,
    pub long_desc: AsOption<String>,
    pub usage: AsOption<String>,
    pub examples: Vec<String>,
    pub authority: bool,
    pub only_guilds: bool,
    pub bucket: AsOption<String>,
    pub sub_commands: Vec<Ident>,
}
