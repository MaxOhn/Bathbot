use crate::{arguments::MarkovChannelArgs, MySQL};
use markov::Chain;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
    utils::{content_safe, ContentSafeOptions},
};

#[command]
#[only_in("guild")]
#[description = "Impersonate anyone's messages. \
If a channel is specified, I will only consider data from that channel.\n\
Credits to [Taha Hawa](https://gitlab.com/tahahawa/discord-markov-bot/)"]
#[usage = "[channel id / mention] [amount of messages]"]
pub fn hivemind(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = MarkovChannelArgs::new(args);
    let amount = args.amount;
    let strings = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.impersonate_strings(None, args.channel)?
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
        for line in chain.str_iter_for(amount) {
            let _ = msg.channel_id.say(
                &ctx.http,
                content_safe(&ctx.cache, &line, &ContentSafeOptions::default()),
            );
        }
    } else {
        let _ = msg.reply(ctx, "They haven't said anything");
    }
    Ok(())
}
