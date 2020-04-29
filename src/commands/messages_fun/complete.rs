use crate::{Guilds, MySQL};

use markov::Chain;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
    utils::{content_safe, ContentSafeOptions},
};

#[command]
#[only_in("guild")]
#[description = "Finishing the given sentence"]
#[usage = "[the beginning of a sentence]"]
pub async fn complete(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
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
    if args.is_empty() {
        msg.channel_id
            .say(
                &ctx.http,
                "Give me the beginning of a sentence so I can finish it",
            )
            .await?;
        return Ok(());
    }
    let args_str = args.rest().to_lowercase();
    let args = args_str.as_str();
    let strings = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.contain_string(args)?
    };
    if strings.is_empty() {
        msg.reply(
            &ctx,
            "I haven't seen any message containing this sequence of words yet",
        )
        .await?;
    } else {
        let mut chain: Chain<String> = Chain::new();
        let mut i = 0;
        let len = strings.len();
        for s in strings.iter() {
            let s = s.to_lowercase();
            if let Some(idx) = s.find(args) {
                if s.len() > idx + args.len() {
                    let suffix: &str = &s[idx + args.len()..];
                    chain.feed_str(suffix);
                    if i == len / 4 {
                        let _ = msg.channel_id.broadcast_typing(&ctx.http).await;
                        i = 0;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        let _ = msg.channel_id.broadcast_typing(&ctx.http);
        for mut line in chain.str_iter_for((strings.len() / 10 + 1).min(15)) {
            line.insert_str(0, args);
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    content_safe(&ctx.cache, &line, &ContentSafeOptions::default()).await,
                )
                .await;
        }
    }
    Ok(())
}
