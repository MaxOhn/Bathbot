use crate::CommandFun;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma, Mut},
    Ident, Lifetime, Lit, Stmt, Type,
};

macro_rules! propagate_err {
    ($res:expr) => {{
        match $res {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        }
    }};
}

pub struct Argument {
    pub mutable: Option<Mut>,
    pub name: Ident,
    pub kind: Type,
}

impl ToTokens for Argument {
    fn to_tokens(&self, stream: &mut TokenStream2) {
        let Argument {
            mutable,
            name,
            kind,
        } = self;
        stream.extend(quote! {
            #mutable #name: #kind
        });
    }
}

pub struct Parenthesised<T>(pub Punctuated<T, Comma>);

impl<T: Parse> Parse for Parenthesised<T> {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        Ok(Parenthesised(content.parse_terminated(T::parse)?))
    }
}

pub trait LitExt {
    fn to_str(&self) -> String;
    fn to_ident(&self) -> Ident;
}

impl LitExt for Lit {
    fn to_str(&self) -> String {
        match self {
            Lit::Str(s) => s.value(),
            Lit::ByteStr(s) => unsafe { String::from_utf8_unchecked(s.value()) },
            Lit::Char(c) => c.value().to_string(),
            Lit::Byte(b) => (b.value() as char).to_string(),
            _ => panic!("values must be a (byte)string or a char"),
        }
    }

    #[inline]
    fn to_ident(&self) -> Ident {
        Ident::new(&self.to_str(), self.span())
    }
}

pub trait IdentExt: Sized {
    fn to_uppercase(&self) -> Self;
    fn with_suffix(&self, suf: &str) -> Ident;
}

impl IdentExt for Ident {
    #[inline]
    fn to_uppercase(&self) -> Self {
        format_ident!("{}", self.to_string().to_uppercase())
    }

    #[inline]
    fn with_suffix(&self, suffix: &str) -> Ident {
        format_ident!("{}_{}", self.to_string().to_uppercase(), suffix)
    }
}

pub struct AsOption<T>(pub Option<T>);

impl<T: ToTokens> ToTokens for AsOption<T> {
    fn to_tokens(&self, stream: &mut TokenStream2) {
        match &self.0 {
            Some(o) => stream.extend(quote!(Some(#o))),
            None => stream.extend(quote!(None)),
        }
    }
}

impl<T> std::ops::Deref for AsOption<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Default for AsOption<T> {
    #[inline]
    fn default() -> Self {
        AsOption(None)
    }
}

#[inline]
fn validation_stmt(actual: &Type, intended: &Type) -> Stmt {
    parse_quote! {
        static_assertions::assert_type_eq_all!(#actual, #intended);
    }
}

pub fn create_declaration_validations(fun: &mut CommandFun) {
    if fun.args.len() != 2 {
        panic!("command function requires ctx and msg as arguments");
    }

    let intended_types: Vec<Type> = vec![
        parse_quote!(std::sync::Arc<crate::Context>), // first arg
        parse_quote!(&twilight::model::channel::Message), // second arg
        parse_quote!(crate::BotResult<()>),           // return value
    ];
    let validations = fun
        .args
        .iter()
        .map(|arg| &arg.kind)
        .chain(std::iter::once(&fun.ret))
        .zip(intended_types.iter())
        .map(|(actual, intended)| validation_stmt(actual, intended));
    for validation in validations {
        fun.body.insert(0, validation);
    }
}

#[inline]
pub fn populate_fut_lifetimes_on_refs(args: &mut Vec<Argument>) {
    for arg in args {
        if let Type::Reference(ref mut reference) = arg.kind {
            reference.lifetime = Some(Lifetime::new("'fut", Span::call_site()));
        }
    }
}
