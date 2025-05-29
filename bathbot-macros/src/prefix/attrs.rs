use syn::{Attribute, Error, Ident, Meta, MetaList, Result, Token};

use crate::{
    bucket::Bucket,
    flags::Flags,
    util::{AsOption, LitOrConst, PunctuatedExt},
};

pub struct CommandAttrs {
    pub aliases: Box<[LitOrConst]>,
    pub desc: Option<LitOrConst>,
    pub help: AsOption<LitOrConst>,
    pub usage: AsOption<LitOrConst>,
    pub examples: Box<[LitOrConst]>,
    pub bucket: AsOption<Bucket>,
    pub flags: Flags,
    pub group: Option<Ident>,
}

impl CommandAttrs {
    pub fn parse_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut aliases = None;
        let mut desc = None;
        let mut help = None;
        let mut usage = None;
        let mut examples = None;
        let mut bucket = None;
        let mut flags = None;
        let mut group = None;

        for attr in attrs {
            let meta_list = match attr.meta {
                Meta::List(ref list) => list,
                Meta::Path(_) | Meta::NameValue(_) => {
                    let message = "expected attribute of the form `#[attr_name(...)]`";

                    return Err(Error::new_spanned(attr, message));
                }
            };

            const EXPECTED: &str = r#"expected "alias", "desc", "help", "usage", "example", "flags", "bucket", or "group""#;

            let name = meta_list
                .path
                .get_ident()
                .ok_or_else(|| Error::new_spanned(attr, EXPECTED))?;

            let name_str = name.to_string();

            match name_str.as_str() {
                "alias" | "aliases" => aliases = Some(parse_all(meta_list)?.into_boxed_slice()),
                "example" | "examples" => examples = Some(parse_all(meta_list)?.into_boxed_slice()),
                "desc" => desc = parse_one(meta_list)?,
                "help" => help = parse_one(meta_list)?,
                "usage" => usage = parse_one(meta_list)?,
                "bucket" => bucket = Some(meta_list.parse_args()?),
                "flags" => flags = Some(meta_list.parse_args()?),
                "group" => group = Some(meta_list.parse_args()?),
                _ => return Err(Error::new_spanned(name, EXPECTED)),
            }
        }

        Ok(Self {
            aliases: aliases.unwrap_or_default(),
            desc,
            help: AsOption(help),
            usage: AsOption(usage),
            examples: examples.unwrap_or_default(),
            bucket: AsOption(bucket),
            flags: flags.unwrap_or_default(),
            group,
        })
    }
}

fn parse_all(list: &MetaList) -> Result<Vec<LitOrConst>> {
    list.parse_args_with(Vec::<LitOrConst>::parse_separated_nonempty::<Token![,]>)
}

fn parse_one(list: &MetaList) -> Result<Option<LitOrConst>> {
    let mut list = parse_all(list)?.into_iter();

    match (list.next(), list.next()) {
        (first, None) => Ok(first),
        (_, Some(second)) => Err(Error::new_spanned(
            second,
            "expected a single string literal",
        )),
    }
}
