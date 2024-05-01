use eyre::{Result, WrapErr};
use twilight_http::{
    request::{Request, RequestBuilder},
    routing::Route,
};
use twilight_model::application::command::Command as TwilightCommand;

use super::Context;
use crate::core::{
    commands::interaction::twilight_command::{Command, IntegrationType, InteractionContextType},
    BotConfig,
};

impl Context {
    pub async fn set_global_commands(
        &self,
        mut cmds: Vec<Command>,
    ) -> Result<Vec<TwilightCommand>> {
        let route = Route::SetGlobalCommands {
            application_id: self.data.application_id.get(),
        };

        add_integrations_and_contexts(&mut cmds);

        send_command_request(self, route, &cmds).await
    }

    pub async fn set_guild_commands(&self, mut cmds: Vec<Command>) -> Result<Vec<TwilightCommand>> {
        let route = Route::SetGuildCommands {
            application_id: self.data.application_id.get(),
            guild_id: BotConfig::get().dev_guild.get(),
        };

        add_integrations_and_contexts(&mut cmds);

        send_command_request(self, route, &cmds).await
    }
}

fn add_integrations_and_contexts(cmds: &mut [Command]) {
    let mut integrations = vec![IntegrationType::GuildInstall];
    let mut contexts = vec![InteractionContextType::Guild, InteractionContextType::BotDm];

    if std::env::args().all(|arg| arg != "--no-user-installs") {
        integrations.push(IntegrationType::UserInstall);
        contexts.push(InteractionContextType::PrivateChannel);
    };

    for cmd in cmds {
        cmd.integration_types.extend_from_slice(&integrations);
        cmd.contexts.extend_from_slice(&contexts);
    }
}

async fn send_command_request(
    ctx: &Context,
    route: Route<'_>,
    cmds: &[Command],
) -> Result<Vec<TwilightCommand>> {
    let req = Request::builder(&route)
        .json(&cmds)
        .map(RequestBuilder::build)
        .wrap_err("Failed to build command request")?;

    ctx.http
        .request(req)
        .await
        .wrap_err("Failed to set commands")?
        .models()
        .await
        .wrap_err("Failed to deserialize commands")
}
