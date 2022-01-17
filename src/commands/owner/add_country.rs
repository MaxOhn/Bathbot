use crate::{
    util::{constants::GENERAL_ISSUE, CountryCode, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[short_desc("Add a country for snipe commands")]
#[usage("[country code] [country name]")]
#[owner()]
async fn addcountry(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let (code, country) = match args.next() {
                Some(code) => {
                    if code.len() != 2 || code.chars().any(|c| !c.is_ascii_uppercase()) {
                        let content =
                            "The country code must consist of two uppercase ASCII characters.";

                        return msg.error(&ctx, content).await;
                    } else if args.rest().is_empty() {
                        let content = "After the country code you must specify the country name";

                        return msg.error(&ctx, content).await;
                    }

                    (CountryCode::from(code), args.rest().to_owned())
                }
                _ => {
                    let content = "You must specify two arguments: \
                        First the country code, then the country name.";

                    return msg.error(&ctx, content).await;
                }
            };

            _addcountry(ctx, CommandData::Message { msg, args, num }, code, country).await
        }
        CommandData::Interaction { command } => super::slash_owner(ctx, *command).await,
    }
}

pub(super) async fn _addcountry(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    code: CountryCode,
    country: String,
) -> BotResult<()> {
    let valid_country = country.split(' ').all(|word| {
        let mut chars = word.chars();

        chars.next().map_or(false, |c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_lowercase())
    });

    if !valid_country {
        let content =
            "Every word in the country name should start with a capital letter followed by lowercase letters";

        return data.error(&ctx, content).await;
    }

    if let Some(name) = ctx.get_country(&code) {
        let content = format!(
            "The country code `{code}` is already available for `{name}`."
        );

        return data.error(&ctx, content).await;
    }

    let insert_fut = ctx.psql().insert_snipe_country(&country, code.as_str());

    if let Err(why) = insert_fut.await {
        let _ = data.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let content = format!("Successfuly added country `{country}` (`{code}`)");
    ctx.add_country(country, code);
    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
