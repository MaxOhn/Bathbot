use syn::{
    punctuated::Punctuated, spanned::Spanned, token::Comma, Attribute, Error, Ident, Lit, Meta,
    NestedMeta, Result as SynResult,
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
        let meta = attr.parse_meta()?;
        let span = meta.span();
        let name = meta.path().get_ident().map(|i| i.to_string());

        let nested = match meta {
            Meta::Path(_) | Meta::NameValue(_) => {
                let message = format!("expected attribute of the form `#[{:?}(...)]`", meta.path());

                return Err(Error::new(attr.span(), message));
            }
            Meta::List(list) => list.nested,
        };

        match name.as_deref() {
            Some("alias") | Some("aliases") => {
                aliases = parse_all(nested).map_err(|m| Error::new(span, m))?
            }
            Some("example") | Some("examples") => {
                examples = parse_all(nested).map_err(|m| Error::new(span, m))?
            }
            Some("desc") => desc = parse_one(nested).map_err(|m| Error::new(span, m))?,
            Some("help") => help = parse_one(nested).map_err(|m| Error::new(span, m))?,
            Some("usage") => usage = parse_one(nested).map_err(|m| Error::new(span, m))?,
            Some("group") => {
                group = Some(parse_meta_ident(nested).map_err(|m| Error::new(span, m))?)
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

fn parse_all(nested: Punctuated<NestedMeta, Comma>) -> Result<Vec<String>, &'static str> {
    nested.iter().map(parse_meta_lit).collect()
}

fn parse_one(nested: Punctuated<NestedMeta, Comma>) -> Result<Option<String>, &'static str> {
    if nested.len() > 1 {
        return Err("expected a single string literal");
    }

    nested.iter().map(parse_meta_lit).next().transpose()
}

fn parse_meta_lit(meta: &NestedMeta) -> Result<String, &'static str> {
    match meta {
        NestedMeta::Lit(Lit::Str(lit)) => Ok(lit.value()),
        _ => Err("expected string literal"),
    }
}

fn parse_meta_ident(mut nested: Punctuated<NestedMeta, Comma>) -> Result<Ident, &'static str> {
    let ident = nested
        .pop()
        .map(|pair| pair.into_value())
        .and_then(|meta| match meta {
            NestedMeta::Meta(Meta::Path(mut path)) => path
                .segments
                .pop()
                .map(|pair| pair.into_value())
                .map(|seg| seg.ident),
            _ => None,
        });

    ident.ok_or("expected identifier")
}
