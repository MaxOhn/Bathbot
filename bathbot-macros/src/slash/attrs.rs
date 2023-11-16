use proc_macro2::Span;
use syn::{Attribute, Error, LitBool, LitStr, Result};

use crate::{bucket::Bucket, flags::Flags, util::AsOption};

pub(super) struct CommandAttrs {
    pub(super) bucket: AsOption<Bucket>,
    pub(super) flags: Flags,
    pub(super) name_lit: LitStr,
}

impl CommandAttrs {
    pub fn parse_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut bucket = None;
        let mut flags = None;
        let mut name_lit = None;

        for attr in attrs {
            if attr.path().is_ident("bucket") {
                bucket = Some(attr.parse_args()?);
            } else if attr.path().is_ident("flags") {
                flags = Some(attr.parse_args()?);
            } else if attr.path().is_ident("command") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("name") {
                        name_lit = Some(meta.value()?.parse()?);
                    } else if meta.path.is_ident("desc") | meta.path.is_ident("help") {
                        let _: LitStr = meta.value()?.parse()?;
                    } else if meta.path.is_ident("dm_permission") {
                        let _: LitBool = meta.value()?.parse()?;
                    } else {
                        return Err(meta.error(
                            "`SlashCommand` expected `name`, `desc`, `help`, or `dm_permission` \
                            command meta",
                        ));
                    }

                    Ok(())
                })?;
            }
        }

        Ok(Self {
            bucket: AsOption(bucket),
            flags: flags.unwrap_or_default(),
            name_lit: name_lit.ok_or_else(|| {
                Error::new(Span::call_site(), "missing #[command(name = \"...\")]")
            })?,
        })
    }
}
