use std::sync::Arc;

use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    util::{builder::MessageBuilder, constants::GENERAL_ISSUE, ApplicationCommandExt},
    BotResult, Context,
};

use super::OwnerAddCountry;

pub async fn addcountry(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    country: OwnerAddCountry,
) -> BotResult<()> {
    let OwnerAddCountry { mut code, name } = country;

    code.make_ascii_uppercase();

    if let Some(name) = ctx.get_country(&code) {
        let content = format!("The country code `{code}` is already available for `{name}`.");
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let valid_country = name.split(' ').all(|word| {
        let mut chars = word.chars();

        chars.next().map_or(false, |c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_lowercase())
    });

    if !valid_country {
        let content =
            "Every word in the country name should start with a capital letter followed by lowercase letters";
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let insert_fut = ctx.psql().insert_snipe_country(&name, code.as_str());

    if let Err(err) = insert_fut.await {
        let _ = command.error_callback(&ctx, GENERAL_ISSUE).await;

        return Err(err);
    }

    let content = format!("Successfuly added country `{name}` (`{code}`)");
    ctx.add_country(name, code.into());
    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
