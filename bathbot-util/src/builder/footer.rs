use twilight_model::channel::message::embed::EmbedFooter;

#[derive(Clone)]
pub struct FooterBuilder {
    pub icon_url: Option<String>,
    pub text: String,
}

impl FooterBuilder {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon_url: None,
        }
    }

    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        self.icon_url = Some(icon_url.into());

        self
    }

    pub fn build(self) -> EmbedFooter {
        EmbedFooter {
            icon_url: self.icon_url,
            proxy_icon_url: None,
            text: self.text,
        }
    }
}

pub trait IntoFooterBuilder {
    fn into(self) -> FooterBuilder;
}

impl IntoFooterBuilder for &str {
    #[inline]
    fn into(self) -> FooterBuilder {
        FooterBuilder {
            icon_url: None,
            text: self.to_owned(),
        }
    }
}

impl IntoFooterBuilder for String {
    #[inline]
    fn into(self) -> FooterBuilder {
        FooterBuilder {
            icon_url: None,
            text: self,
        }
    }
}

impl IntoFooterBuilder for FooterBuilder {
    #[inline]
    fn into(self) -> FooterBuilder {
        self
    }
}
