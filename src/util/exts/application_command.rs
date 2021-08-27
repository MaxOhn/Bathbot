use crate::{BotResult, Error};

use twilight_model::{
    application::interaction::{application_command::CommandDataOption, ApplicationCommand},
    id::UserId,
};

pub trait ApplicationCommandExt {
    fn user_id(&self) -> BotResult<UserId>;
    fn username(&self) -> BotResult<&str>;
    fn yoink_options(&mut self) -> Vec<CommandDataOption>;
}

impl ApplicationCommandExt for ApplicationCommand {
    fn user_id(&self) -> BotResult<UserId> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.id)
            .ok_or(Error::MissingSlashAuthor)
    }

    fn username(&self) -> BotResult<&str> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.name.as_str())
            .ok_or(Error::MissingSlashAuthor)
    }

    fn yoink_options(&mut self) -> Vec<CommandDataOption> {
        std::mem::replace(&mut self.data.options, Vec::new())
    }
}
