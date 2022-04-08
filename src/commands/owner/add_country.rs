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
    let OwnerAddCountry { code, name } = country;

    // TODO: Capitalize code?

    if let Some(name) = ctx.get_country(&code) {
        let content = format!("The country code `{code}` is already available for `{name}`.");

        return command.error(&ctx, content).await;
    }

    let valid_country = name.split(' ').all(|word| {
        let mut chars = word.chars();

        chars.next().map_or(false, |c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_lowercase())
    });

    if !valid_country {
        let content =
            "Every word in the country name should start with a capital letter followed by lowercase letters";

        return command.error(&ctx, content).await;
    }

    let insert_fut = ctx.psql().insert_snipe_country(&name, code.as_str());

    if let Err(why) = insert_fut.await {
        let _ = command.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let content = format!("Successfuly added country `{country}` (`{code}`)");
    ctx.add_country(country, code);
    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
