use crate::{embeds::EmbedData, util::constants::INVITE_LINK};

#[derive(Clone)]
pub struct InviteEmbed {
    fields: Vec<(String, String, bool)>,
}

impl InviteEmbed {
    pub fn new() -> Self {
        let fields = vec![(
            "Invite me to your server!".to_owned(),
            INVITE_LINK.to_owned(),
            false,
        )];
        Self { fields }
    }
}

impl EmbedData for InviteEmbed {
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
