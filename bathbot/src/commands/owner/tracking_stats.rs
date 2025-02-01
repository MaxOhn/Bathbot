use bathbot_util::{fields, EmbedBuilder, MessageBuilder};
use eyre::Result;
use metrics::Key;
use rosu_v2::model::GameMode;

use crate::{
    core::Context,
    tracking::OsuTracking,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

pub async fn trackingstats(command: InteractionCommand) -> Result<()> {
    command.defer(false).await?;

    let stats = OsuTracking::stats();

    let modes_str = [
        GameMode::Osu,
        GameMode::Taiko,
        GameMode::Catch,
        GameMode::Mania,
    ]
    .map(GameMode::as_str);

    let key = Key::from_name("bathbot.osu_tracking_hit");
    let mut mode_hits = [0; 4];

    Context::get().metrics.collect_counters(&key, |key, value| {
        for label in key.labels() {
            if label.key() != "mode" {
                continue;
            }

            let label_value = label.value();

            let opt = modes_str
                .iter()
                .zip(mode_hits.iter_mut())
                .find(|(&mode, _)| mode == label_value);

            let Some((_, count)) = opt else { return };
            *count += value;
        }
    });

    let [hits_osu, hits_taiko, hits_catch, hits_mania] = mode_hits;
    let hits = format!(
        "`osu!: {hits_osu}` • `osu!taiko: {hits_taiko}` • \
        `osu!catch: {hits_catch}` • `osu!mania: {hits_mania}`"
    );

    let counts = format!(
        "`Total entries: {}` • `Unique users: {}` • `Channels: {}`\n\
        `osu!: {}` • `osu!taiko: {}` • `osu!catch: {}` • `osu!mania: {}`",
        stats.total,
        stats.unique_users,
        stats.channels,
        stats.count_osu,
        stats.count_taiko,
        stats.count_catch,
        stats.count_mania,
    );

    let fields = fields![
        "Hits since reboot".to_owned(), hits, false;
        "Counts".to_owned(), counts, false;
    ];

    let embed = EmbedBuilder::new()
        .fields(fields)
        .title("Tracking statistics:");

    let builder = MessageBuilder::new().embed(embed);
    command.update(builder).await?;

    Ok(())
}
