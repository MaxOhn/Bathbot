use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use twilight_interactions::command::ApplicationCommandData;
use twilight_model::{
    application::command::{CommandOption, CommandType},
    guild::Permissions,
    id::{
        marker::{ApplicationMarker, CommandMarker, CommandVersionMarker, GuildMarker},
        Id,
    },
};

/// Same as [`twilight_model::application::command::Command`] but extended to
/// work with the new user installs.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Command {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<Id<ApplicationMarker>>,
    pub default_member_permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dm_permission: Option<bool>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<Id<GuildMarker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id<CommandMarker>>,
    #[serde(rename = "type")]
    pub kind: CommandType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsfw: Option<bool>,
    #[serde(default)]
    pub options: Vec<CommandOption>,
    pub version: Id<CommandVersionMarker>,
}

impl From<ApplicationCommandData> for Command {
    fn from(item: ApplicationCommandData) -> Self {
        Command {
            application_id: None,
            guild_id: None,
            name: item.name,
            name_localizations: item.name_localizations,
            default_member_permissions: item.default_member_permissions,
            dm_permission: item.dm_permission,
            description: item.description,
            description_localizations: item.description_localizations,
            id: None,
            kind: CommandType::ChatInput,
            options: item.options.into_iter().map(CommandOption::from).collect(),
            version: Id::new(1),
            nsfw: None,
        }
    }
}
