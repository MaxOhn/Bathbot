use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Attribute, Ident, Result};

use crate::util::AsOption;

pub fn parse_bucket(attrs: &[Attribute]) -> Result<AsOption<Bucket>> {
    attrs
        .iter()
        .find(|attr| match attr.path.get_ident() {
            Some(ident) => ident == "bucket",
            None => return false,
        })
        .map(|a| a.parse_args())
        .transpose()
        .map(|b| AsOption(b.map(|bucket| Bucket { bucket })))
}

pub struct Bucket {
    bucket: Ident,
}

impl ToTokens for Bucket {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.bucket;
        tokens.extend(quote!(crate::core::buckets::BucketName::#ident));
    }
}
