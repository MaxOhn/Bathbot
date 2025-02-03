use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    Error, Expr, ExprLit, Lit, LitBool, LitStr, Meta, Result, Token,
};

use crate::{flags::Flags, util::PunctuatedExt};

pub struct CommandAttrs {
    pub name: LitStr,
    pub dm_permission: Option<LitBool>,
    pub flags: Flags,
}

impl Parse for CommandAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let metas = Vec::<Meta>::parse_separated_nonempty::<Token![,]>(input)?;

        let mut attr_name = None;
        let mut dm_permission = None;
        let mut flags = None;

        for meta in metas {
            match meta {
                Meta::NameValue(meta) => {
                    if meta.path.is_ident("name") {
                        let Expr::Lit(ExprLit {
                            lit: Lit::Str(lit), ..
                        }) = meta.value
                        else {
                            return Err(Error::new_spanned(meta.value, "expected string literal"));
                        };

                        attr_name = Some(lit);
                    } else if meta.path.is_ident("dm_permission") {
                        let Expr::Lit(ExprLit {
                            lit: Lit::Bool(lit),
                            ..
                        }) = meta.value
                        else {
                            return Err(Error::new_spanned(meta.value, "expected boolean literal"));
                        };

                        dm_permission = Some(lit);
                    } else {
                        return Err(Error::new_spanned(
                            meta.value,
                            "expected `name` or `dm_permission`",
                        ));
                    }
                }
                Meta::List(meta) => {
                    if meta.path.is_ident("flags") {
                        flags = Some(meta.parse_args()?);
                    } else {
                        return Err(Error::new_spanned(meta.path, "expected `flags`"));
                    }
                }
                Meta::Path(_) => {}
            }
        }

        Ok(Self {
            name: attr_name
                .ok_or_else(|| Error::new(Span::call_site(), "must specify `name = \"...\"`"))?,
            dm_permission,
            flags: flags.unwrap_or_default(),
        })
    }
}
