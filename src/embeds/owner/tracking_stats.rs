use crate::{
    embeds::{EmbedData, Footer},
    tracking::TrackingStats,
};

use chrono::{DateTime, Utc};

#[derive(Clone)]
pub struct TrackingStatsEmbed {
    description: String,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl TrackingStatsEmbed {
    pub fn new(stats: TrackingStats) -> Self {
        let (user_id, mode) = stats.next_pop;
        let description = format!(
            "Currently tracking: {}\n\
            Tracked users: {}\n\
            Interval per user: {}s\n\
            Minimal cooldown: {}ms\n\
            Next pop: {} | {}\n\n\
            Wait interval: {}s\n\
            Milliseconds per user: {}ms\n\
            Next pop amount: {}\n\
            Current delay: {}ms",
            stats.tracking,
            stats.len,
            stats.interval,
            stats.cooldown,
            user_id,
            mode,
            stats.wait_interval,
            stats.ms_per_track,
            stats.amount,
            stats.delay
        );
        Self {
            description,
            footer: Footer::new("Last pop"),
            timestamp: stats.last_pop,
        }
    }
}

impl EmbedData for TrackingStatsEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }
}
