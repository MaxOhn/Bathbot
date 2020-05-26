use crate::{arguments::MarkovChannelArgs, util::discord, Guilds, MySQL};
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
pub async fn hivemind(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    {
        let data = ctx.data.read().await;
        let guilds = data.get::<Guilds>().expect("Could not get Guilds");
        if !guilds.get(&msg.guild_id.unwrap()).unwrap().message_tracking {
            let response = msg
                .channel_id
                .say(
                    &ctx.http,
                    "No messages tracked on this guild yet. \
                     To enable message tracking, use the `<enabletracking` command first.",
                )
                .await?;
            discord::reaction_deletion(&ctx, response, msg.author.id).await;
            return Ok(());
        }
    }
    let args = MarkovChannelArgs::new(args);
    let channels = if let Some(ref channel) = args.channel {
        let guild_lock = msg.guild(ctx).await.expect("Could not get guild of msg");
        let guild = guild_lock.read().await;
        if guild.channels.keys().all(|id| id != channel) {
            let response = msg
                .channel_id
                .say(
                    ctx,
                    "If a channel is specified, it must be in this server. \
                     The given channel was not found.",
                )
                .await?;
            discord::reaction_deletion(&ctx, response, msg.author.id).await;
            return Ok(());
        }
        vec![channel.0]
    } else {
        let guild_lock = msg.guild(ctx).await.expect("Could not get guild of msg");
        let guild = guild_lock.read().await;
        guild.channels.keys().map(|id| id.0).collect()
    };
    let mut strings = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.impersonate_strings(None, Some(channels))?
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
                let _ = msg.channel_id.broadcast_typing(&ctx.http).await;
                i = 0;
            } else {
                i += 1;
            }
        }
        for line in chain.str_iter_for(args.amount) {
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    content_safe(&ctx.cache, &line, &ContentSafeOptions::default()).await,
                )
                .await;
        }
    } else {
        let response = msg.reply(ctx, "They haven't said anything").await?;
        discord::reaction_deletion(&ctx, response, msg.author.id).await;
    }
    Ok(())
}
