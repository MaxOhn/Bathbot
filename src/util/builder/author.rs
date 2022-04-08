use twilight_model::channel::embed::EmbedAuthor;

#[derive(Clone)]
pub struct AuthorBuilder(EmbedAuthor);

impl AuthorBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self(EmbedAuthor {
            name: name.into(),
            url: None,
            icon_url: None,
            proxy_icon_url: None,
        })
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.0.url = Some(url.into());

        self
    }

    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        let icon_url = icon_url.into();
        self.0.icon_url = Some(icon_url);

        self
    }

    pub fn build(self) -> EmbedAuthor {
        self.0
    }

    pub fn as_author(&self) -> &EmbedAuthor {
        &self.0
    }
}

impl From<AuthorBuilder> for EmbedAuthor {
    #[inline]
    fn from(author: AuthorBuilder) -> Self {
        author.build()
    }
}

impl From<&AuthorBuilder> for EmbedAuthor {
    #[inline]
    fn from(author: &AuthorBuilder) -> Self {
        author.as_author().to_owned()
    }
}
