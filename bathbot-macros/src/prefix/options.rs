use syn::{
    punctuated::Punctuated, spanned::Spanned, token::Comma, Attribute, Error, Ident, LitStr, Meta,
    MetaList, Result as SynResult,
};

use crate::{
    bucket::{parse_bucket, Bucket},
    flags::{parse_flags, Flags},
    util::AsOption,
};

pub struct Options {
    pub aliases: Vec<String>,
    pub desc: Option<String>,
    pub help: AsOption<String>,
    pub usage: AsOption<String>,
    pub examples: Vec<String>,
    pub bucket: AsOption<Bucket>,
    pub flags: Flags,
    pub group: Option<Ident>,
}

pub fn parse_options(attrs: &[Attribute]) -> SynResult<Options> {
    let mut aliases = Vec::new();
    let mut desc = None;
    let mut help = None;
    let mut usage = None;
    let mut examples = Vec::new();
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
            Some("alias") | Some("aliases") => {
                aliases = parse_all(meta_list).map_err(|m| Error::new(span, m))?
            }
            Some("example") | Some("examples") => {
                examples = parse_all(meta_list).map_err(|m| Error::new(span, m))?
            }
            Some("desc") => desc = parse_one(meta_list).map_err(|m| Error::new(span, m))?,
            Some("help") => help = parse_one(meta_list).map_err(|m| Error::new(span, m))?,
            Some("usage") => usage = parse_one(meta_list).map_err(|m| Error::new(span, m))?,
            Some("group") => {
                group = Some(parse_meta_ident(meta_list).map_err(|m| Error::new(span, m))?)
            }
            Some("flags" | "bucket") => {}
            _ => {
                let message = r#"expected "alias", "desc", "help", "usage", "example", "flags", "bucket", or "group""#;

                return Err(Error::new(span, message));
            }
        }
    }

    Ok(Options {
        aliases,
        desc,
        help: AsOption(help),
        usage: AsOption(usage),
        examples,
        bucket: parse_bucket(attrs)?,
        flags: parse_flags(attrs)?,
        group,
    })
}

fn parse_all(list: &MetaList) -> Result<Vec<String>, &'static str> {
    let res = list
        .parse_args_with(Punctuated::<LitStr, Comma>::parse_separated_nonempty)
        .map_err(|_| "expected list of literals")?
        .into_iter()
        .map(|lit| lit.value())
        .collect();

    Ok(res)
}

fn parse_one(list: &MetaList) -> Result<Option<String>, &'static str> {
    let mut list = parse_all(list).map_err(|_| "expected string literal")?;

    if list.len() > 1 {
        return Err("expected a single string literal");
    }

    Ok(list.pop())
}

fn parse_meta_ident(list: &MetaList) -> Result<Ident, &'static str> {
    list.parse_args().map_err(|_| "expected identifier")
}
