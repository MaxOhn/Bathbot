use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Data, DeriveInput, Error, Fields, Result};

use self::attributes::Attributes;

mod attributes;

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

    let Attributes { jump_idx, no_multi } = Attributes::try_from(attrs)?;

    let reaction_vec = quote!(crate::pagination::ReactionVec);
    let emote = quote!(crate::util::Emote);

    let jump_idx = match jump_idx {
        Some(lit) => quote!(self.#lit),
        None => quote!(None),
    };

    let multi = if no_multi {
        quote!(self.single_reaction(vec))
    } else {
        quote! {
            if self.pages.total_pages > 8 {
                vec.push(#emote::MultiStepBack);
            }

            self.single_reaction(vec);

            if self.pages.total_pages > 8 {
                vec.push(#emote::MultiStep);
            }
        }
    };

    let tokens = quote! {
        impl crate::pagination::BasePagination for #ident {
            fn msg(&self) -> &Message {
                &self.msg
            }

            fn pages(&self) -> &Pages {
                &self.pages
            }

            fn pages_mut(&mut self) -> &mut Pages {
                &mut self.pages
            }

            fn jump_index(&self) -> Option<usize> {
                #jump_idx
            }

            fn multi_reaction(&self, vec: &mut #reaction_vec) {
                #multi
            }

            fn my_pos_reaction(&self, vec: &mut #reaction_vec) {
                if self.jump_index().is_some() {
                    vec.push(#emote::MyPosition);
                }
            }
        }
    };

    Ok(tokens)
}
