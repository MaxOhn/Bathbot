use twilight_model::{
    application::interaction::{application_command::CommandDataOption, ApplicationCommand},
    id::UserId,
};

pub trait ApplicationCommandExt {
    fn user_id(&self) -> Option<UserId>;
    fn username(&self) -> Option<&str>;
    fn yoink_options(&mut self) -> Vec<CommandDataOption>;
}

impl ApplicationCommandExt for ApplicationCommand {
    fn user_id(&self) -> Option<UserId> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.id)
    }

    fn username(&self) -> Option<&str> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.name.as_str())
    }

    fn yoink_options(&mut self) -> Vec<CommandDataOption> {
        std::mem::replace(&mut self.data.options, Vec::new())
    }
}
