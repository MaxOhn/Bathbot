use crate::{
    embeds::{EmbedFields, Footer},
    tracking::TrackingStats,
};

use chrono::{DateTime, Utc};

pub struct TrackingStatsEmbed {
    title: String,
    fields: EmbedFields,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl TrackingStatsEmbed {
    pub fn new(stats: TrackingStats) -> Self {
        let (user_id, mode) = stats.next_pop;

        let fields = vec![
            field!("Currently tracking", stats.tracking.to_string(), true),
            field!("Interval per user", format!("{}s", stats.interval), true),
            field!("Minimal cooldown", format!("{}ms", stats.cooldown), true),
            field!("Current delay", format!("{}ms", stats.delay), true),
            field!("Wait interval", format!("{}s", stats.wait_interval), true),
            field!(
                "Milliseconds per user",
                format!("{}ms", stats.ms_per_track),
                true
            ),
            field!("Next pop", format!("{} | {}", user_id, mode), true),
            field!("Next pop amount", stats.amount.to_string(), true),
        ];

        let title = format!("Tracked users: {} | queue: {}", stats.users, stats.queue);

        Self {
            fields,
            footer: Footer::new("Last pop"),
            timestamp: stats.last_pop,
            title,
        }
    }
}

impl_builder!(TrackingStatsEmbed {
    fields,
    footer,
    timestamp,
    title,
});
