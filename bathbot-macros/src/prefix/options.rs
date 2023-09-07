use proc_macro2::Span;
use syn::{
    spanned::Spanned, Attribute, Error, Ident, LitStr, Meta, MetaList, Result as SynResult, Result,
    Token,
};

use crate::{
    bucket::{parse_bucket, Bucket},
    flags::{parse_flags, Flags},
    util::{AsOption, PunctuatedExt},
};

pub struct Options {
    pub aliases: Box<[Box<str>]>,
    pub desc: Option<Box<str>>,
    pub help: AsOption<Box<str>>,
    pub usage: AsOption<Box<str>>,
    pub examples: Box<[Box<str>]>,
    pub bucket: AsOption<Bucket>,
    pub flags: Flags,
    pub group: Option<Ident>,
}

pub fn parse_options(attrs: &[Attribute]) -> SynResult<Options> {
    let mut aliases = None;
    let mut desc = None;
    let mut help = None;
    let mut usage = None;
    let mut examples = None;
    let mut group = None;

    for attr in attrs {
        let meta = &attr.meta;
        let span = meta.span();
        let name = meta.path().get_ident().map(|i| i.to_string());

        let meta_list = match meta {
            Meta::Path(_) | Meta::NameValue(_) => {
                let message = format!("expected attribute of the form `#[{:?}(...)]`", meta.path());

                return Err(Error::new(attr.span(), message));
            }
            Meta::List(list) => list,
        };

        match name.as_deref() {
            Some("alias") | Some("aliases") => aliases = Some(parse_all(meta_list)?),
            Some("example") | Some("examples") => examples = Some(parse_all(meta_list)?),
            Some("desc") => desc = parse_one(meta_list, span)?,
            Some("help") => help = parse_one(meta_list, span)?,
            Some("usage") => usage = parse_one(meta_list, span)?,
            Some("group") => group = Some(meta_list.parse_args()?),
            Some("flags" | "bucket") => {}
            _ => {
                let message = r#"expected "alias", "desc", "help", "usage", "example", "flags", "bucket", or "group""#;

                return Err(Error::new(span, message));
            }
        }
    }

    Ok(Options {
        aliases: aliases.unwrap_or_default(),
        desc,
        help: AsOption(help),
        usage: AsOption(usage),
        examples: examples.unwrap_or_default(),
        bucket: parse_bucket(attrs)?,
        flags: parse_flags(attrs)?,
        group,
    })
}

fn parse_all(list: &MetaList) -> Result<Box<[Box<str>]>> {
    let res = list
        .parse_args_with(Vec::<LitStr>::parse_separated_nonempty::<Token![,]>)?
        .into_iter()
        .map(|lit| lit.value().into_boxed_str())
        .collect();

    Ok(res)
}

fn parse_one(list: &MetaList, err_span: Span) -> Result<Option<Box<str>>> {
    let list = parse_all(list)?;

    if list.len() > 1 {
        return Err(Error::new(err_span, "expected a single string literal"));
    }

    Ok(list.into_vec().pop())
}
