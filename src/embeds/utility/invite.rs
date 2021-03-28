use crate::{embeds::Footer, util::constants::INVITE_LINK};

pub struct InviteEmbed {
    description: &'static str,
    footer: Footer,
    title: &'static str,
}

impl InviteEmbed {
    pub fn new() -> Self {
        Self {
            description: INVITE_LINK,
            footer: Footer::new("The initial prefix will be <"),
            title: "Invite me to your server!",
        }
    }
}

impl_into_builder!(InviteEmbed {
    description,
    footer,
    title,
});
