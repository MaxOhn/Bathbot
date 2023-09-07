use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Ident, Result,
};

use crate::util::AsOption;

// TODO: remove
pub fn parse_bucket(attrs: &[Attribute]) -> Result<AsOption<Bucket>> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("bucket"))
        .map(|a| a.parse_args())
        .transpose()
        .map(|b| AsOption(b.map(|bucket| Bucket { bucket })))
}

pub struct Bucket {
    bucket: Ident,
}

impl Parse for Bucket {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse().map(|bucket| Self { bucket })
    }
}

impl ToTokens for Bucket {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.bucket;
        tokens.extend(quote!(crate::core::buckets::BucketName::#ident));
    }
}
