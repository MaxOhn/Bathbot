use crate::{
    embeds::{EmbedData, Footer},
    util::constants::INVITE_LINK,
};

pub struct InviteEmbed {
    title: &'static str,
    description: &'static str,
    footer: Footer,
}

impl InviteEmbed {
    pub fn new() -> Self {
        let title = "Invite me to your server!";
        let description = INVITE_LINK;
        let footer = Footer::new("The initial prefix will be <");
        Self {
            title,
            description,
            footer,
        }
    }
}

impl EmbedData for InviteEmbed {
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }
    fn description(&self) -> Option<&str> {
        Some(self.description)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
