use crate::{
    arguments::Args,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Add a country for snipe commands")]
#[usage("[country name] [country code]")]
#[owner()]
async fn addcountry(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let (country, code) = match (args.next(), args.next()) {
        (Some(country), Some(code)) => (country.to_owned(), code.into()),
        _ => {
            let content = "You must specify two arguments: \
            first the country name, then the country code";

            return msg.error(&ctx, content).await;
        }
    };

    ctx.add_country(country, code);

    // TODO: Database

    todo!()
}
