use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    spanned::Spanned,
    token::{Mut, Underscore},
    Attribute, Block, Error, FnArg, Ident, Pat, PatType, Result, ReturnType, Stmt, Token, Type,
    Visibility,
};

use crate::util::Parenthesised;

pub struct CommandFun {
    // #[...]
    pub attrs: Vec<Attribute>,
    // pub / nothing
    pub vis: Visibility,
    // name
    pub name: Ident,
    // (...)
    pub args: Vec<Argument>,
    // -> ...
    pub ret: Box<Type>,
    // { ... }
    pub body: Vec<Stmt>,
}

impl Parse for CommandFun {
    fn parse(input: ParseStream) -> Result<Self> {
        // #[...]
        let attrs = input.call(Attribute::parse_outer)?;

        // pub / nothing
        let vis = input.parse::<Visibility>()?;

        // async fn
        input.parse::<Token![async]>()?;
        input.parse::<Token![fn]>()?;

        // name
        let name = input.parse::<Ident>()?;

        // arguments
        let args = {
            let Parenthesised::<FnArg>(args) = input.parse()?;

            args.into_iter()
                .map(Argument::try_from)
                .collect::<Result<Vec<_>>>()?
        };

        // -> BotResult<()>
        let ret = match input.parse::<ReturnType>()? {
            ReturnType::Type(_, t) if t == parse_quote! { BotResult<()> } => t,
            ReturnType::Type(_, t) => {
                return Err(Error::new(t.span(), "expected return type `BotResult<()>`"))
            }
            _ => {
                return Err(Error::new(
                    name.span(),
                    "expected return type `BotResult<()>`",
                ))
            }
        };

        // { ... }
        let block = input.parse::<Block>()?;

        if block.stmts.is_empty() {
            let message = "block must return `BotResult<()>`";

            return Err(Error::new(block.span(), message));
        }

        let body = block.stmts;

        Ok(Self {
            attrs,
            vis,
            name,
            args,
            ret,
            body,
        })
    }
}

impl ToTokens for CommandFun {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            vis,
            name,
            args,
            ret,
            body,
            ..
        } = self;

        tokens.extend(quote! {
            #vis async fn #name<'fut>(#(#args),*) -> #ret {
                #(#body)*
            }
        });
    }
}

pub enum ArgumentName {
    Ident(Ident),
    Wild(Underscore),
}

impl ToTokens for ArgumentName {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ArgumentName::Ident(ident) => tokens.extend(quote!(#ident)),
            ArgumentName::Wild(underscore) => tokens.extend(quote!(#underscore)),
        }
    }
}

pub struct Argument {
    pub mutability: Option<Mut>,
    pub name: ArgumentName,
    pub ty: Box<Type>,
}

impl ToTokens for Argument {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            mutability,
            name,
            ty,
        } = self;

        tokens.extend(quote!(#mutability #name: #ty));
    }
}

impl TryFrom<FnArg> for Argument {
    type Error = Error;

    fn try_from(arg: FnArg) -> Result<Self> {
        match arg {
            FnArg::Receiver(_) => Err(Error::new(
                arg.span(),
                "expected arguments of the form `identifier: type`",
            )),
            FnArg::Typed(typed) => {
                let PatType { pat, ty, .. } = typed;

                match *pat {
                    Pat::Ident(id) => Ok(Argument {
                        mutability: id.mutability,
                        name: ArgumentName::Ident(id.ident),
                        ty,
                    }),
                    Pat::Wild(wild) => Ok(Argument {
                        mutability: None,
                        name: ArgumentName::Wild(wild.underscore_token),
                        ty,
                    }),
                    _ => Err(Error::new(
                        pat.span(),
                        "expected either _ or identifier before `:`",
                    )),
                }
            }
        }
    }
}
