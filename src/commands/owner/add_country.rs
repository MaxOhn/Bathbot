use std::sync::Arc;

use eyre::Result;

use crate::{
    util::{
        builder::MessageBuilder, constants::GENERAL_ISSUE, interaction::InteractionCommand,
        InteractionCommandExt,
    },
    Context,
};

use super::OwnerAddCountry;

pub async fn addcountry(
    ctx: Arc<Context>,
    command: InteractionCommand,
    country: OwnerAddCountry,
) -> Result<()> {
    let OwnerAddCountry { mut code, name } = country;

    code.make_ascii_uppercase();

    if let Some(name) = ctx.huismetbenen().get_country(&code).await {
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

    if let Err(err) = ctx.huismetbenen().add_country(&code, &name).await {
        let _ = command.error_callback(&ctx, GENERAL_ISSUE).await;

        return Err(err.wrap_err("failed to insert huismetbenen country"));
    }

    let content = format!("Successfuly added country `{name}` (`{code}`)");
    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
