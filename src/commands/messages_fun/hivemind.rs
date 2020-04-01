use crate::{arguments::MarkovChannelArgs, Guilds, MySQL};
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
#[usage = "[channel id / mention] [amount of messages] [no-urls]"]
pub async fn hivemind(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    {
        let data = ctx.data.read().await;
        let guilds = data.get::<Guilds>().expect("Could not get Guilds");
        if !guilds.get(&msg.guild_id.unwrap()).unwrap().message_tracking {
            msg.channel_id
                .say(
                    &ctx.http,
                    "No messages tracked on this guild yet. \
                To enable message tracking, use the `<enabletracking` command first.",
                )
                .await?;
            return Ok(());
        }
    }
    let args = MarkovChannelArgs::new(args);
    let amount = args.amount;
    let mut strings = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.impersonate_strings(None, args.channel)?
    };
    if args.no_url {
        strings.retain(|s| !s.starts_with("http"));
    }
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
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    content_safe(&ctx.cache, &line, &ContentSafeOptions::default()).await,
                )
                .await;
        }
    } else {
        msg.reply(ctx, "They haven't said anything").await?;
    }
    Ok(())
}
