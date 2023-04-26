use twilight_model::guild::Permissions;

use super::interaction::{InteractionCommand, InteractionComponent, InteractionModal};
use crate::core::commands::CommandOrigin;

pub trait CheckPermissions {
    fn permissions(&self) -> Option<Permissions>;

    fn can_read_history(&self) -> bool {
        self.has_permission_to(Permissions::READ_MESSAGE_HISTORY)
    }

    fn can_attach_file(&self) -> bool {
        self.has_permission_to(Permissions::ATTACH_FILES)
    }

    fn can_create_thread(&self) -> bool {
        self.has_permission_to(Permissions::CREATE_PUBLIC_THREADS)
    }

    fn can_view_channel(&self) -> bool {
        self.has_permission_to(Permissions::VIEW_CHANNEL)
    }

    fn has_permission_to(&self, permission: Permissions) -> bool {
        self.permissions().map_or(true, |p| p.contains(permission))
    }
}

impl CheckPermissions for CommandOrigin<'_> {
    fn permissions(&self) -> Option<Permissions> {
        match self {
            CommandOrigin::Message { permissions, .. } => *permissions,
            CommandOrigin::Interaction { command } => command.permissions,
        }
    }
}

impl CheckPermissions for InteractionCommand {
    fn permissions(&self) -> Option<Permissions> {
        self.permissions
    }
}

impl CheckPermissions for InteractionComponent {
    fn permissions(&self) -> Option<Permissions> {
        self.permissions
    }
}

impl CheckPermissions for InteractionModal {
    fn permissions(&self) -> Option<Permissions> {
        self.permissions
    }
}
