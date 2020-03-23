use crate::{arguments::MarkovUserArgs, MySQL};

use itertools::Itertools;
use markov::Chain;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
    utils::{content_safe, ContentSafeOptions},
};

pub fn impersonate_send(
    with_markov: bool,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let args = match MarkovUserArgs::new(args, ctx, msg.guild_id.unwrap()) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id.say(&ctx.http, err_msg)?;
            return Ok(());
        }
    };
    let mut strings = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.impersonate_strings(Some(args.user), None)?
    };
    if args.no_url {
        strings.retain(|s| !s.starts_with("http"));
    }
    if !strings.is_empty() {
        if with_markov {
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
            msg.channel_id
                .say(&ctx.http, strings.into_iter().take(args.amount).join("\n"))?;
        }
    } else {
        let _ = msg.reply(
            &ctx,
            "Either they've never said anything, or I haven't seen them",
        );
    }
    Ok(())
}

#[command]
#[only_in("guild")]
#[description = "Impersonate someone's messages.\n\
Credits to [Taha Hawa](https://gitlab.com/tahahawa/discord-markov-bot/)"]
#[usage = "[user id / mention] [amount of messages] [-no-urls]"]
pub fn impersonate(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    impersonate_send(true, ctx, msg, args)
}

#[command]
#[only_in("guild")]
#[description = "Repeat random messages that the user said at some point"]
#[usage = "[user id / mention] [amount of messages] [-no-urls]"]
#[aliases("rh")]
pub fn randomhistory(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    impersonate_send(false, ctx, msg, args)
}
