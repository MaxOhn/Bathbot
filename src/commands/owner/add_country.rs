use crate::{
    arguments::Args,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use smallstr::SmallString;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Add a country for snipe commands")]
#[usage("[country name] [country code]")]
#[owner()]
async fn addcountry(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let (country, code) = match (args.next(), args.next()) {
        (Some(country), Some(code)) => {
            if code.len() != 2 || code.chars().any(|c| !c.is_ascii_uppercase()) {
                let content = "The country code must consist of two uppercase ASCII characters.";

                return msg.error(&ctx, content).await;
            }

            (country.to_owned(), SmallString::<[u8; 2]>::from(code))
        }
        _ => {
            let content = "You must specify two arguments: \
            first the country name, then the country code";

            return msg.error(&ctx, content).await;
        }
    };

    if let Some(name) = ctx.get_country(&code) {
        let content = format!(
            "The country code `{}` is already available for `{}`.",
            code, name
        );

        return msg.error(&ctx, content).await;
    }

    let insert_fut = ctx.psql().insert_snipe_country(&country, code.as_str());

    if let Err(why) = insert_fut.await {
        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let content = format!("Successfuly added country `{}` (`{}`)", country, code);
    ctx.add_country(country, code);

    msg.send_response(&ctx, content).await
}
