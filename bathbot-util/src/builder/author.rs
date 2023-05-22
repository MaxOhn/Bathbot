use twilight_model::channel::message::embed::EmbedAuthor;

#[derive(Clone)]
pub struct AuthorBuilder {
    pub icon_url: Option<String>,
    pub name: String,
    pub url: Option<String>,
}

impl AuthorBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: None,
            icon_url: None,
        }
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());

        self
    }

    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        let icon_url = icon_url.into();
        self.icon_url = Some(icon_url);

        self
    }

    pub fn build(self) -> EmbedAuthor {
        EmbedAuthor {
            icon_url: self.icon_url,
            name: self.name,
            proxy_icon_url: None,
            url: self.url,
        }
    }
}

impl From<AuthorBuilder> for EmbedAuthor {
    #[inline]
    fn from(author: AuthorBuilder) -> Self {
        author.build()
    }
}
