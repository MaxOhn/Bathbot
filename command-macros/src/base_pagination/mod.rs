use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_quote, spanned::Spanned, Attribute, Data, DeriveInput, Error, Fields, Lit, Meta,
    NestedMeta, Result,
};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        attrs, ident, data, ..
    } = input;

    let data = match data {
        Data::Struct(s) => s,
        Data::Enum(e) => {
            let message = "`BasePagination` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, message));
        }
        Data::Union(u) => {
            let message = "`BasePagination` can only be derived for structs";

            return Err(Error::new(u.union_token.span, message));
        }
    };

    let named_fields = match data.fields {
        Fields::Named(n) => n.named,
        _ => {
            let message = "Deriving `BasePagination` requires named fields";

            return Err(Error::new(ident.span(), message));
        }
    };

    let valid_msg_field = named_fields.iter().any(|field| match field.ident {
        Some(ref ident) if ident == "msg" => field.ty == parse_quote!(Message),
        _ => false,
    });

    if !valid_msg_field {
        let message = "Deriving `BasePagination` requires a field `msg` of type `Message`";

        return Err(Error::new(ident.span(), message));
    }

    let valid_pages_field = named_fields.iter().any(|field| match field.ident {
        Some(ref ident) if ident == "pages" => field.ty == parse_quote!(Pages),
        _ => false,
    });

    if !valid_pages_field {
        let message = "Deriving `BasePagination` requires a field `pages` of type `Pages`";

        return Err(Error::new(ident.span(), message));
    }

    let (single_step, multi_step) = parse_steps(&attrs)?;

    let multi_step = match multi_step {
        Some(value) => quote!(#value),
        None => quote!(self.pages.per_page),
    };

    let tokens = quote! {
        impl crate::pagination::BasePagination for #ident {
            fn msg(&self) -> &Message {
                &self.msg
            }

            fn pages(&self) -> Pages {
                self.pages
            }

            fn pages_mut(&mut self) -> &mut Pages {
                &mut self.pages
            }

            fn single_step(&self) -> usize {
                #single_step
            }

            fn multi_step(&self) -> usize {
                #multi_step
            }
        }
    };

    Ok(tokens)
}

fn parse_steps(attrs: &[Attribute]) -> Result<(usize, Option<usize>)> {
    let meta_opt = attrs
        .iter()
        .find_map(|attr| (attr.path.get_ident()? == "pagination").then(|| attr))
        .map(Attribute::parse_meta)
        .transpose()?;

    let meta = match meta_opt {
        Some(Meta::List(list)) => list,
        Some(Meta::Path(path)) => return Err(Error::new(path.span(), "Expected a meta list")),
        Some(Meta::NameValue(val)) => return Err(Error::new(val.span(), "Expected a meta list")),
        None => return Ok((1, None)),
    };

    let mut single_step = None;
    let mut multi_step = None;

    for nested in meta.nested {
        let meta = match nested {
            NestedMeta::Meta(Meta::NameValue(val)) => val,
            NestedMeta::Meta(Meta::List(list)) => {
                return Err(Error::new(list.span(), "Expected a name value"))
            }
            NestedMeta::Meta(Meta::Path(path)) => {
                return Err(Error::new(path.span(), "Expected a name value"))
            }
            NestedMeta::Lit(lit) => return Err(Error::new(lit.span(), "Expected a name value")),
        };

        let ident = match meta.path.get_ident() {
            Some(ident) => ident,
            None => return Err(Error::new(meta.path.span(), "Expected ident")),
        };

        let value = match meta.lit {
            Lit::Int(value) => value.base10_parse()?,
            _ => return Err(Error::new(meta.lit.span(), "Expected integer literal")),
        };

        if ident == "single_step" {
            single_step = Some(value);
        } else if ident == "multi_step" {
            multi_step = Some(value);
        } else {
            let message = r#"Expected "single_step" or "multi_step""#;

            return Err(Error::new(ident.span(), message));
        }
    }

    Ok((single_step.unwrap_or(1), multi_step))
}
