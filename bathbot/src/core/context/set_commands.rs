use eyre::{Result, WrapErr};
use twilight_http::{
    request::{Request, RequestBuilder},
    routing::Route,
};
use twilight_model::application::command::Command as TwilightCommand;

use super::Context;
use crate::core::{commands::interaction::twilight_command::Command, BotConfig};

impl Context {
    pub async fn set_global_commands(&self, cmds: &[Command]) -> Result<Vec<TwilightCommand>> {
        let route = Route::SetGlobalCommands {
            application_id: self.data.application_id.get(),
        };

        send_command_request(self, route, cmds).await
    }

    pub async fn set_guild_commands(&self, cmds: &[Command]) -> Result<Vec<TwilightCommand>> {
        let route = Route::SetGuildCommands {
            application_id: self.data.application_id.get(),
            guild_id: BotConfig::get().dev_guild.get(),
        };

        send_command_request(self, route, cmds).await
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
