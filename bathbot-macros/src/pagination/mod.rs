use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parse_quote, parse_quote_spanned, spanned::Spanned, Data, DataStruct, DeriveInput, Error, Expr,
    Field, FieldMutability, FieldValue, Fields, FieldsNamed, GenericArgument, Lit, LitInt, Member,
    Meta, PathArguments, Type, Visibility,
};

pub(super) fn impl_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let DeriveInput {
        attrs: _,
        vis: _,
        ident,
        generics: _,
        data,
    } = input;

    let builder_name = format_ident!("{ident}Builder");

    let data = extract_data(data)
        .map_err(|span| Error::new(span, "Can only derive `PaginationBuilder` for structs"))?;

    let mut fields = extract_fields(data.fields).ok_or_else(|| {
        let msg = "Can only derive `PaginationBuilder` for named fields";

        Error::new_spanned(&ident, msg)
    })?;

    remove_pages_field(&mut fields)?;

    let PagesData {
        per_page,
        pages_len,
    } = extract_pages_data(&fields)?;

    let assigned_fields = fields.named.iter().map(|field| FieldValue {
        attrs: Vec::new(),
        member: Member::Named(field.ident.clone().expect("field must have name")),
        colon_token: field.colon_token,
        expr: parse_quote!(None),
    });

    let finalized_vars = fields.named.iter().map(|field| {
        let name = field.ident.as_ref().expect("field must have name");
        let span = field.span();

        if is_option(&field.ty) {
            quote_spanned! { span => let #name = self. #name .take() }
        } else {
            quote_spanned! {span=>
                let #name = self.
                    #name
                    .take()
                    .expect(concat!("missing ", stringify!(#name)))
            }
        }
    });

    let finalized_fields = fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap());

    let builder_fields = fields.named.iter().map(|field| {
        let ty = &field.ty;

        let ty = if is_option(ty) {
            ty.to_owned()
        } else {
            parse_quote_spanned! { ty.span() => ::core::option::Option< #ty > }
        };

        Field {
            attrs: Vec::new(),
            vis: Visibility::Inherited,
            mutability: FieldMutability::None,
            ident: field.ident.clone(),
            colon_token: field.colon_token,
            ty,
        }
    });

    let builder_methods = fields.named.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let ty = &field.ty;

        let assign = if is_option(ty) {
            quote!(#name)
        } else {
            quote!(Some( #name ))
        };

        quote! {
            pub fn #name(&mut self, #name: #ty) -> &mut Self {
                self. #name = #assign;

                self
            }
        }
    });

    let tokens = quote! {
        impl #ident {
            pub fn builder() -> #builder_name {
                #builder_name {
                    #( #assigned_fields ,)*
                }
            }
        }

        pub struct #builder_name {
            #( #builder_fields ,)*
        }

        impl #builder_name {
            pub fn build(&mut self) -> #ident {
                #( #finalized_vars ;)*

                let pages_len: usize = #pages_len;

                #ident {
                    pages: crate::active::pagination::Pages::new( #per_page, pages_len),
                    #( #finalized_fields ,)*
                }
            }

            #( #builder_methods )*
        }
    };

    Ok(tokens)
}

fn extract_data(data: Data) -> Result<DataStruct, Span> {
    match data {
        Data::Struct(data) => Ok(data),
        Data::Enum(data) => Err(data.enum_token.span),
        Data::Union(data) => Err(data.union_token.span),
    }
}

fn extract_fields(fields: Fields) -> Option<FieldsNamed> {
    match fields {
        Fields::Named(fields) => Some(fields),
        Fields::Unnamed(_) | Fields::Unit => None,
    }
}

fn remove_pages_field(fields: &mut FieldsNamed) -> Result<(), Error> {
    let Some(last) = fields.named.last() else {
        return Err(Error::new_spanned(fields, "must have fields"));
    };

    let valid_name = last.ident.as_ref().is_some_and(|ident| ident == "pages");

    let valid_ty = matches!(
        last.ty,
        Type::Path(ref ty_path) if ty_path.path.is_ident("Pages")
    );

    if !(valid_name && valid_ty) {
        let msg = "last field must be `pages: Pages`";

        return Err(Error::new_spanned(last, msg));
    }

    fields.named.pop();

    Ok(())
}

fn is_option(ty: &Type) -> bool {
    let Type::Path(ty_path) = ty else {
        return false;
    };

    let Some(segment) = ty_path.path.segments.last() else {
        return false;
    };

    if segment.ident != "Option" {
        return false;
    }

    let PathArguments::AngleBracketed(ref path_args) = segment.arguments else {
        return false;
    };

    matches!(path_args.args.first(), Some(GenericArgument::Type(_)))
}

fn extract_pages_data(fields: &FieldsNamed) -> Result<PagesData, Error> {
    fields
        .named
        .iter()
        .find_map(|field| {
            let meta_list = field.attrs.iter().find_map(|attr| match attr.meta {
                Meta::List(ref list) if list.path.is_ident("pagination") => Some(list),
                _ => None,
            })?;

            let mut per_page = None;
            let mut pages_len = None;

            let parse_res = meta_list.parse_nested_meta(|meta| {
                if meta.path.is_ident("per_page") {
                    let value = meta.value()?;
                    per_page = Some(value.parse::<LitInt>()?);

                    Ok(())
                } else if meta.path.is_ident("len") {
                    let value = meta.value()?;

                    pages_len = match value.parse::<Lit>().unwrap() {
                        Lit::Str(lit) => Some(lit.parse()?),
                        Lit::Int(lit) => Some(parse_quote_spanned! { lit.span() => #lit }),
                        _ => return Err(meta.error("expected stringified expression")),
                    };

                    Ok(())
                } else {
                    Err(meta.error("expected attribute `per_page` or `len`"))
                }
            });

            let pages_len = pages_len.unwrap_or_else(|| {
                let name = field.ident.as_ref().unwrap();

                parse_quote_spanned! { field.span() => #name .len() }
            });

            match per_page {
                Some(per_page) => Some(parse_res.map(|_| PagesData {
                    per_page,
                    pages_len,
                })),
                None => Some(Err(Error::new_spanned(
                    field,
                    "must specify attribute `per_page`",
                ))),
            }
        })
        .unwrap_or_else(|| {
            Err(Error::new_spanned(
                fields,
                "one field must be denoted with the `pagination` attribute",
            ))
        })
}

struct PagesData {
    per_page: LitInt,
    pages_len: Expr,
}
