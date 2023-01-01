use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    Error, Lit, LitInt, MetaNameValue, Token,
};

pub struct AttributeList {
    inner: Punctuated<MetaNameValue, Comma>,
}

impl Parse for AttributeList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut inner = Punctuated::new();
        inner.push_value(input.parse()?);

        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            inner.push(input.parse()?);
        }

        Ok(Self { inner })
    }
}

pub struct Attributes {
    pub per_page: LitInt,
    pub entries: LengthOrigin,
}

pub enum LengthOrigin {
    Field(Ident),
    Value(Ident),
}

impl ToTokens for LengthOrigin {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let extend = match self {
            Self::Field(field) => quote!(#field.len()),
            Self::Value(value) => quote!(#value),
        };

        tokens.extend(extend);
    }
}

impl TryFrom<AttributeList> for Attributes {
    type Error = Error;

    fn try_from(list: AttributeList) -> Result<Self, Self::Error> {
        let mut per_page = None;
        let mut entries = None;

        for name_value in list.inner {
            match name_value.path.get_ident() {
                Some(ident) if ident == "per_page" => match name_value.lit {
                    Lit::Int(lit) => per_page = Some(lit),
                    _ => {
                        let message = "Expected an integer";

                        return Err(Error::new(name_value.lit.span(), message));
                    }
                },
                Some(ident) if ident == "entries" => match name_value.lit {
                    Lit::Str(lit) => entries = Some(LengthOrigin::Field(lit.parse()?)),
                    _ => {
                        let message = "Expected a string containing the name of a field";

                        return Err(Error::new(name_value.lit.span(), message));
                    }
                },
                Some(ident) if ident == "total" => match name_value.lit {
                    Lit::Str(lit) => entries = Some(LengthOrigin::Value(lit.parse()?)),
                    _ => {
                        let message = "Expected a string containing the name of a field";

                        return Err(Error::new(name_value.lit.span(), message));
                    }
                },
                _ => {
                    let message = "Expected `per_page`, `entries`, or `total` as name";

                    return Err(Error::new(name_value.span(), message));
                }
            }
        }

        let per_page = per_page
            .ok_or_else(|| Error::new(Span::call_site(), "Expected `#[pages(per_page = ...)]`"))?;

        let entries = entries.ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "Expected either `#[pages(entries = \"...\")]` or `#[pages(total = \"...\")]`",
            )
        })?;

        Ok(Self { per_page, entries })
    }
}
