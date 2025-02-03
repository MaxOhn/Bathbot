use eyre::{Result, WrapErr};
use twilight_model::{application::command::Command, oauth::ApplicationIntegrationType};

use super::Context;
use crate::core::BotConfig;

impl Context {
    #[cold]
    pub async fn set_global_commands(mut cmds: Vec<Command>) -> Result<Vec<Command>> {
        let integrations = vec![
            ApplicationIntegrationType::GuildInstall,
            ApplicationIntegrationType::UserInstall,
        ];

        for cmd in cmds.iter_mut() {
            if cmd.integration_types.is_none() {
                cmd.integration_types = Some(integrations.clone());
            } else {
                warn!(command = cmd.name, "Command integrations already set");
            }
        }

        Context::interaction()
            .set_global_commands(&cmds)
            .await
            .wrap_err("Failed to set commands")?
            .models()
            .await
            .wrap_err("Failed to deserialize commands")
    }

    #[cold]
    pub async fn set_guild_commands(cmds: Vec<Command>) -> Result<Vec<Command>> {
        Context::interaction()
            .set_guild_commands(BotConfig::get().dev_guild, &cmds)
            .await
            .wrap_err("Failed to set commands")?
            .models()
            .await
            .wrap_err("Failed to deserialize commands")
    }
}
