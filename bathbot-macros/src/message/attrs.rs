use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    Error, Expr, ExprLit, Lit, LitBool, LitStr, Meta, Result, Token,
};

use crate::{
    flags::Flags,
    util::{AsOption, PunctuatedExt},
};

pub struct CommandAttrs {
    pub name: LitStr,
    pub dm_permission: AsOption<LitBool>,
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
                    let Some(name) = meta.path.get_ident() else {
                        continue;
                    };

                    if name == "name" {
                        let Expr::Lit(ExprLit {
                            lit: Lit::Str(lit), ..
                        }) = meta.value
                        else {
                            return Err(Error::new_spanned(meta.value, "expected string literal"));
                        };

                        attr_name = Some(lit);
                    } else if name == "dm_permission" {
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
                            name,
                            "expected `name` or `dm_permission`",
                        ));
                    }
                }
                Meta::List(meta) => {
                    let Some(name) = meta.path.get_ident() else {
                        continue;
                    };

                    if name == "flags" {
                        flags = Some(meta.parse_args()?);
                    } else {
                        return Err(Error::new_spanned(name, "expected `flags`"));
                    }
                }
                Meta::Path(_) => {}
            }
        }

        Ok(Self {
            name: attr_name
                .ok_or_else(|| Error::new(Span::call_site(), "must specify `name = \"...\"`"))?,
            dm_permission: AsOption(dm_permission),
            flags: flags.unwrap_or_else(Flags::new),
        })
    }
}
