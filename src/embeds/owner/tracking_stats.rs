use crate::{
    embeds::{EmbedData, EmbedFields, Footer},
    tracking::TrackingStats,
};

use chrono::{DateTime, Utc};

pub struct TrackingStatsEmbed {
    title: Option<String>,
    fields: EmbedFields,
    footer: Option<Footer>,
    timestamp: DateTime<Utc>,
}

impl TrackingStatsEmbed {
    pub fn new(stats: TrackingStats) -> Self {
        let (user_id, mode) = stats.next_pop;
        let fields = smallvec![
            (
                "Currently tracking".to_owned(),
                stats.tracking.to_string(),
                true,
            ),
            (
                "Interval per user".to_owned(),
                format!("{}s", stats.interval),
                true,
            ),
            (
                "Minimal cooldown".to_owned(),
                format!("{}ms", stats.cooldown),
                true,
            ),
            (
                "Current delay".to_owned(),
                format!("{}ms", stats.delay),
                true,
            ),
            (
                "Wait interval".to_owned(),
                format!("{}s", stats.wait_interval),
                true,
            ),
            (
                "Milliseconds per user".to_owned(),
                format!("{}ms", stats.ms_per_track),
                true,
            ),
            (
                "Next pop".to_owned(),
                format!("{} | {}", user_id, mode),
                true,
            ),
            ("Next pop amount".to_owned(), stats.amount.to_string(), true),
        ];
        Self {
            fields,
            footer: Some(Footer::new("Last pop")),
            timestamp: stats.last_pop,
            title: Some(format!(
                "Tracked users: {} | queue: {}",
                stats.users, stats.queue
            )),
        }
    }
}

impl EmbedData for TrackingStatsEmbed {
    fn fields_owned(self) -> Option<EmbedFields> {
        Some(self.fields)
    }
    fn footer_owned(&mut self) -> Option<Footer> {
        self.footer.take()
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }
    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }
}
