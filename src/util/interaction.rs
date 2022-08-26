use twilight_model::{
    application::interaction::{
        application_command::CommandData, message_component::MessageComponentInteractionData,
        modal::ModalInteractionData,
    },
    channel::Message,
    guild::{PartialMember, Permissions},
    id::{
        marker::{ChannelMarker, GuildMarker, InteractionMarker},
        Id,
    },
    user::User,
};

use crate::{BotResult, Error};

use super::Authored;

#[derive(Debug)]
pub struct InteractionCommand {
    pub permissions: Option<Permissions>,
    pub channel_id: Id<ChannelMarker>,
    pub data: Box<CommandData>,
    pub guild_id: Option<Id<GuildMarker>>,
    pub id: Id<InteractionMarker>,
    pub member: Option<PartialMember>,
    pub token: String,
    pub user: Option<User>,
}

#[derive(Debug)]
pub struct InteractionComponent {
    pub permissions: Option<Permissions>,
    pub channel_id: Id<ChannelMarker>,
    pub data: MessageComponentInteractionData,
    pub guild_id: Option<Id<GuildMarker>>,
    pub id: Id<InteractionMarker>,
    pub member: Option<PartialMember>,
    pub message: Message,
    pub token: String,
    pub user: Option<User>,
}

#[derive(Debug)]
pub struct InteractionModal {
    pub permissions: Option<Permissions>,
    pub channel_id: Id<ChannelMarker>,
    pub data: ModalInteractionData,
    pub guild_id: Option<Id<GuildMarker>>,
    pub id: Id<InteractionMarker>,
    pub member: Option<PartialMember>,
    pub message: Option<Message>,
    pub token: String,
    pub user: Option<User>,
}

macro_rules! impl_authored {
    ($($ty:ty,)*) => {
        $(
            impl Authored for $ty {
                fn channel_id(&self) -> Id<ChannelMarker> {
                    self.channel_id
                }

                fn guild_id(&self) -> Option<Id<GuildMarker>> {
                    self.guild_id
                }

                fn user(&self) -> BotResult<&User> {
                    self.member
                        .as_ref()
                        .and_then(|member| member.user.as_ref())
                        .or(self.user.as_ref())
                        .ok_or(Error::MissingAuthor)
                }
            }
        )*
    };
}

impl_authored! {
    InteractionCommand,
    InteractionComponent,
    InteractionModal,
}
