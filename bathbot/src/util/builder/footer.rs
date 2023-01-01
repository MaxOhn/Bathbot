use twilight_model::channel::embed::EmbedFooter;

#[derive(Clone)]
pub struct FooterBuilder(EmbedFooter);

impl FooterBuilder {
    pub fn new(text: impl Into<String>) -> Self {
        Self(EmbedFooter {
            text: text.into(),
            icon_url: None,
            proxy_icon_url: None,
        })
    }

    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        let icon_url = icon_url.into();
        self.0.icon_url = Some(icon_url);

        self
    }

    pub fn build(self) -> EmbedFooter {
        self.0
    }

    pub fn as_footer(&self) -> &EmbedFooter {
        &self.0
    }
}

pub trait IntoEmbedFooter {
    fn into(self) -> EmbedFooter;
}

impl IntoEmbedFooter for EmbedFooter {
    #[inline]
    fn into(self) -> EmbedFooter {
        self
    }
}

impl IntoEmbedFooter for &str {
    #[inline]
    fn into(self) -> EmbedFooter {
        EmbedFooter {
            icon_url: None,
            proxy_icon_url: None,
            text: self.to_owned(),
        }
    }
}

impl IntoEmbedFooter for String {
    #[inline]
    fn into(self) -> EmbedFooter {
        EmbedFooter {
            icon_url: None,
            proxy_icon_url: None,
            text: self,
        }
    }
}

impl IntoEmbedFooter for FooterBuilder {
    #[inline]
    fn into(self) -> EmbedFooter {
        self.build()
    }
}

impl IntoEmbedFooter for &FooterBuilder {
    #[inline]
    fn into(self) -> EmbedFooter {
        self.as_footer().to_owned()
    }
}
