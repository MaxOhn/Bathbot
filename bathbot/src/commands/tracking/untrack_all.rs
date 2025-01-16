use bathbot_macros::command;
use bathbot_util::MessageBuilder;
use eyre::Result;
use rosu_v2::model::GameMode;

use crate::{core::commands::CommandOrigin, tracking::OsuTracking, util::ChannelExt};

#[command]
#[desc("Untrack all users in a channel")]
#[help(
    "Stop notifying a channel about new plays in any user's top100.\n\
    If you only want to untrack all users of a single mode, \
    provide the mode as argument."
)]
#[usage("[osu / mania / taiko / ctb]")]
#[example("", "mania")]
#[flags(AUTHORITY, ONLY_GUILDS, SKIP_DEFER)]
#[group(Tracking)]
async fn prefix_untrackall(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let mode = match args.next() {
        Some("osu") | Some("o") | Some("standard") | Some("s") => Some(GameMode::Osu),
        Some("mania") | Some("m") => Some(GameMode::Mania),
        Some("taiko") | Some("t") => Some(GameMode::Taiko),
        Some("ctb") | Some("c") | Some("catch") => Some(GameMode::Catch),
        None => None,
        _ => {
            let content = "If an argument is provided, \
                it must be either `osu`, `mania`, `taiko`, or `ctb`.";

            msg.error(content).await?;

            return Ok(());
        }
    };

    untrackall(msg.into(), mode).await
}

pub async fn untrackall(orig: CommandOrigin<'_>, mode: Option<GameMode>) -> Result<()> {
    let channel_id = orig.channel_id();

    OsuTracking::remove_channel(channel_id, mode).await;

    let content = "Untracked all users in this channel";
    let builder = MessageBuilder::new().embed(content);
    orig.create_message(builder).await?;

    Ok(())
}
