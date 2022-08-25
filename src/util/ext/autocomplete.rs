use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_model::{
    application::command::CommandOptionChoice,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{core::Context, util::interaction::InteractionAutoComplete};

pub trait AutocompleteExt {
    /// Ackowledge the autocomplete and respond immediatly.
    fn callback(
        &self,
        ctx: &Context,
        choices: Vec<CommandOptionChoice>,
    ) -> ResponseFuture<EmptyBody>;
}

impl AutocompleteExt for InteractionAutoComplete {
    #[inline]
    fn callback(
        &self,
        ctx: &Context,
        choices: Vec<CommandOptionChoice>,
    ) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            choices: Some(choices),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .exec()
    }
}
