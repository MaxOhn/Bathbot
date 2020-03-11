use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{thread, time::Duration};

fn song_send(lyrics: &[&str], delay: u64, ctx: &mut Context, msg: &Message) -> CommandResult {
    let delay = Duration::from_millis(delay);
    msg.channel_id.say(&ctx.http, lyrics[0])?;
    for line in lyrics.iter().skip(1) {
        thread::sleep(delay);
        msg.channel_id.say(&ctx.http, format!("♫ {} ♫", line))?;
    }
    Ok(())
}

#[command]
#[description = "Making me sing https://youtu.be/xpkkakkDhN4?t=65"]
#[bucket = "songs"]
pub fn bombsaway(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "Tick tick tock and it's bombs awayyyy",
        "Come ooon, it's the only way",
        "Save your-self for a better dayyyy",
        "No, no, we are falling dooo-ooo-ooo-ooown",
        "I know, you know - this is over",
        "Tick tick tock and it's bombs awayyyy",
        "Now we're falling -- now we're falling doooown",
    ];
    song_send(lyrics, 3000, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/BjFWk0ncr70?t=12"]
#[bucket = "songs"]
pub fn catchit(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "This song is one you won't forget",
        "It will get stuck -- in your head",
        "If it does, then you can't blame me",
        "Just like I said - too catchy",
    ];
    song_send(lyrics, 3500, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/_yWU0lFghxU?t=54"]
#[bucket = "songs"]
pub fn ding(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "Oh-oh-oh, hübsches Ding",
        "Ich versteck' mein' Ehering",
        "Klinglingeling, wir könnten's bring'n",
        "Doch wir nuckeln nur am Drink",
        "Oh-oh-oh, hübsches Ding",
        "Du bist Queen und ich bin King",
        "Wenn ich dich seh', dann muss ich sing'n:",
        "Tingalingaling, you pretty thing!",
    ];
    song_send(lyrics, 3000, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/0jgrCKhxE1s?t=77"]
#[bucket = "songs"]
pub fn fireandflames(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "So far away we wait for the day-yay",
        "For the lives all so wasted and gooone",
        "We feel the pain of a lifetime lost in a thousand days",
        "Through the fire and the flames we carry ooooooon",
    ];
    song_send(lyrics, 3500, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/psuRGfAaju4?t=25"]
#[bucket = "songs"]
pub fn fireflies(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "You would not believe your eyes",
        "If ten million fireflies",
        "Lit up the world as I fell asleep",
        "'Cause they'd fill the open air",
        "And leave teardrops everywhere",
        "You'd think me rude, but I would just stand and -- stare",
    ];
    song_send(lyrics, 3000, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/la9C0n7jSsI"]
#[bucket = "songs"]
pub fn flamingo(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "How many shrimps do you have to eat",
        "before you make your skin turn pink?",
        "Eat too much and you'll get sick",
        "Shrimps are pretty rich",
    ];
    song_send(lyrics, 2500, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/SyJMQg3spck?t=43"]
#[bucket = "songs"]
pub fn pretender(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "What if I say I'm not like the others?",
        "What if I say I'm not just another oooone of your plays?",
        "You're the pretender",
        "What if I say that I will never surrender?",
    ];
    song_send(lyrics, 3500, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/hjGZLnja1o8?t=41"]
#[bucket = "songs"]
#[aliases("1273")]
pub fn rockefeller(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "1 - 2 - 7 - 3,",
        "down the Rockefeller street.",
        "Life is marchin' on, do you feel that?",
        "1 - 2 - 7 - 3,",
        "down the Rockefeller street.",
        "Everything is more than surreal",
    ];
    song_send(lyrics, 2500, ctx, msg)
}

#[command]
#[description = "Making me sing https://youtu.be/DT6tpUbWOms?t=47"]
#[bucket = "songs"]
pub fn tijdmachine(ctx: &mut Context, msg: &Message) -> CommandResult {
    let lyrics = &[
        "Als ik denk aan al die dagen,",
        "dat ik mij zo heb misdragen.",
        "Dan denk ik, - had ik maar een tijdmachine -- tijdmachine",
        "Maar die heb ik niet,",
        "dus zal ik mij gedragen,",
        "en zal ik blijven sparen,",
        "sparen voor een tijjjdmaaachine.",
    ];
    song_send(lyrics, 2500, ctx, msg)
}
