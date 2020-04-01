use crate::{util::discord, BootTime, CommandCounter};

use chrono::{DateTime, Utc};
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
    utils::Colour,
};
use std::{fmt::Write, iter::FromIterator};

#[command]
#[description = "Let me show you my most popular commands \
                 since my last reboot"]
async fn commands(ctx: &mut Context, msg: &Message) -> CommandResult {
    let response = {
        let symbols = ["♔", "♕", "♖", "♗", "♘", "♙"];
        let mut description = String::with_capacity(128);
        description.push_str("```\n");
        let data = ctx.data.read().await;
        let counter = data
            .get::<CommandCounter>()
            .expect("Could not get CommandCounter");
        let mut vec: Vec<_> = Vec::from_iter(counter);
        vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
        let len = vec
            .iter()
            .take(10)
            .fold(0, |max, (name, _)| max.max(name.len()));
        for (i, (name, amount)) in vec.into_iter().take(10).enumerate() {
            let _ = writeln!(
                description,
                "{:>2} {:1} # {:<len$} => {}",
                i + 1,
                symbols.get(i).unwrap_or_else(|| &""),
                name,
                amount,
                len = len
            );
        }
        description.push_str("```");
        let boot_time: &DateTime<Utc> = data.get::<BootTime>().expect("Could not get BootTime");
        msg.channel_id
            .send_message(&ctx.http, |m| {
                m.embed(|e| {
                    e.footer(|f| f.text("I've been counting since"))
                        .timestamp(boot_time)
                        .color(Colour::DARK_GREEN)
                        .author(|a| a.name("Most popular commands:"))
                        .description(description)
                })
            })
            .await?
    };

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}
