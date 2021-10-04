use std::mem;

use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

pub trait ApplicationCommandExt {
    fn yoink_options(&mut self) -> Vec<CommandDataOption>;
}

impl ApplicationCommandExt for ApplicationCommand {
    fn yoink_options(&mut self) -> Vec<CommandDataOption> {
        mem::take(&mut self.data.options)
    }
}
