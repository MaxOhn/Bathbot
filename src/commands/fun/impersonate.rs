use crate::{arguments::MarkovUserArgs, MySQL};

use markov::Chain;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
    utils::{content_safe, ContentSafeOptions},
};

#[command]
#[only_in("guild")]
#[description = "Impersonate someone's messages.\n\
Credits to [Taha Hawa](https://gitlab.com/tahahawa/discord-markov-bot/)"]
#[usage = "[user id / mention] [amount of messages]"]
pub fn impersonate(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = match MarkovUserArgs::new(args, ctx, msg.guild_id.unwrap()) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id.say(&ctx.http, err_msg)?;
            return Ok(());
        }
    };
    let strings = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.impersonate_strings(Some(args.user), None)?
    };
    if !strings.is_empty() {
        let mut chain: Chain<String> = Chain::new();
        let mut i = 0;
        let len = strings.len();
        for s in strings {
            chain.feed_str(&s);
            if i == len / 4 {
                let _ = msg.channel_id.broadcast_typing(&ctx.http);
                i = 0;
            } else {
                i += 1;
            }
        }
        let _ = msg.channel_id.broadcast_typing(&ctx.http);
        for line in chain.str_iter_for(args.amount) {
            let _ = msg.channel_id.say(
                &ctx.http,
                content_safe(&ctx.cache, &line, &ContentSafeOptions::default()),
            );
        }
    } else {
        let _ = msg.reply(
            &ctx,
            "Either they've never said anything, or I haven't seen them",
        );
    }
    Ok(())
}
