use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let DeriveInput { ident, data, .. } = input;

    let data = match data {
        Data::Struct(s) => s,
        Data::Enum(e) => {
            let message = "`EmbedData` can only be derived for structs";

            return Err(Error::new(e.enum_token.span, message));
        }
        Data::Union(u) => {
            let message = "`EmbedData` can only be derived for structs";

            return Err(Error::new(u.union_token.span, message));
        }
    };

    let named_fields = match data.fields {
        Fields::Named(n) => n.named,
        _ => {
            let message = "Deriving `EmbedData` requires named fields";

            return Err(Error::new(ident.span(), message));
        }
    };

    let mut author = TokenStream::new();
    let mut color = TokenStream::new();
    let mut description = TokenStream::new();
    let mut fields = TokenStream::new();
    let mut footer = TokenStream::new();
    let mut image = TokenStream::new();
    let mut timestamp = TokenStream::new();
    let mut title = TokenStream::new();
    let mut thumbnail = TokenStream::new();
    let mut url = TokenStream::new();

    for field in named_fields {
        let ident = match field.ident {
            Some(ident) => ident,
            None => {
                let message = "Deriving `EmbedData` requires named fields";

                return Err(Error::new(ident.span(), message));
            }
        };

        let ident_str = ident.to_string();

        match ident_str.as_str() {
            "author" => author = quote!(.author(self.author)),
            "color" => color = quote!(.color(self.color)),
            "description" => description = quote!(.description(self.description)),
            "fields" => fields = quote!(.fields(self.fields)),
            "footer" => footer = quote!(.footer(self.footer)),
            "image" => image = quote!(.image(self.image)),
            "timestamp" => timestamp = quote!(.timestamp(self.timestamp)),
            "title" => title = quote!(.title(self.title)),
            "thumbnail" => thumbnail = quote!(.thumbnail(self.thumbnail)),
            "url" => url = quote!(.url(self.url)),
            _ => {
                let message = "Invalid field name for `EmbedData`, must be `author`, `color`, `description`, `fields`, `footer`, `image`, `timestamp`, `title`, `thumbnail`, or `url`";

                return Err(Error::new(ident.span(), message));
            }
        }
    }

    let tokens = quote! {
        impl crate::embeds::EmbedData for #ident {
            fn build(self) -> ::twilight_model::channel::embed::Embed {
                crate::util::builder::EmbedBuilder::new()
                    #author
                    #color
                    #description
                    #fields
                    #footer
                    #image
                    #timestamp
                    #title
                    #thumbnail
                    #url
                    .build()
            }
        }
    };

    Ok(tokens)
}
