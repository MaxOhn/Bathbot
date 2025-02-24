use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Ident, Result,
    parse::{Parse, ParseStream},
};

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
