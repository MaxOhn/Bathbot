use crate::Osu;
use rosu::{
    backend::requests::{OsuRequest, UserRecentRequest},
    models::Score,
};
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[tokio::main]
#[command]
#[aliases("r")]
async fn recent(ctx: &mut Context, msg: &Message) -> CommandResult {
    let req = UserRecentRequest::with_username("Jodehh").limit(1);
    let mut data = ctx.data.write();
    let osu = data.get_mut::<Osu>().expect("Could not get osu client");
    let mut osu_req: OsuRequest<Score> = osu.prepare_request(req);
    let recent: Score = osu_req
        .queue()
        .await
        .expect("Could not queue UserRecentRequest")
        .pop()
        .unwrap();
    let content = format!(
        "Most recent play of {}: {} max combo with {} 300s",
        recent.username, recent.max_combo, recent.count300
    );
    let _ = msg.channel_id.say(&ctx.http, &content);
    Ok(())
}
