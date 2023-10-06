use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_quote,
    token::{Mut, Underscore},
    Attribute, Block, Error, Ident, Result, ReturnType, Token, Type, Visibility,
};

use crate::util::PunctuatedExt;

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
    pub body: Block,
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

        // ( ... )
        let args = {
            let content;
            parenthesized!(content in input);

            // arguments
            Vec::parse_terminated::<Token![,]>(&content)?
        };

        // -> Result<()>
        let ret = match input.parse::<ReturnType>()? {
            ReturnType::Type(_, t) if t == parse_quote! { Result<()> } => t,
            ReturnType::Type(_, t) => {
                return Err(Error::new_spanned(t, "expected return type `Result<()>`"))
            }
            _ => {
                return Err(Error::new_spanned(
                    name,
                    "expected return type `Result<()>`",
                ))
            }
        };

        // { ... }
        let body = input.parse()?;

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
            attrs: _,
            vis,
            name,
            args,
            ret,
            body,
        } = self;

        tokens.extend(quote! {
            #vis async fn #name<'fut>(#(#args),*) -> #ret #body
        });
    }
}

pub enum ArgumentName {
    Ident(Ident),
    Wild(Underscore),
}

impl Parse for ArgumentName {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![_]) {
            input.parse().map(Self::Wild)
        } else {
            input.parse().map(Self::Ident)
        }
    }
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

impl Parse for Argument {
    fn parse(input: ParseStream) -> Result<Self> {
        let mutability = input.peek(Token![mut]).then(|| input.parse()).transpose()?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;

        Ok(Self {
            mutability,
            name,
            ty,
        })
    }
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
