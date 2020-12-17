use crate::{
    embeds::{EmbedData, Footer},
    util::constants::INVITE_LINK,
};

pub struct InviteEmbed {
    title: &'static str,
    description: &'static str,
    footer: Option<Footer>,
}

impl InviteEmbed {
    pub fn new() -> Self {
        let title = "Invite me to your server!";
        let description = INVITE_LINK;
        let footer = Footer::new("The initial prefix will be <");
        Self {
            title,
            description,
            footer: Some(footer),
        }
    }
}

impl EmbedData for InviteEmbed {
    fn title_owned(&mut self) -> Option<String> {
        Some(self.title.to_owned())
    }
    fn description_owned(&mut self) -> Option<String> {
        Some(self.description.to_owned())
    }
    fn footer_owned(&mut self) -> Option<Footer> {
        self.footer.take()
    }
}
