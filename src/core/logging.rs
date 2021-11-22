use std::fmt;

use time::{format_description::FormatItem, macros::format_description};
use tracing::{Event, Subscriber};
use tracing_appender::{
    non_blocking::{NonBlocking, WorkerGuard},
    rolling,
};
use tracing_subscriber::{
    fmt::{
        format::Writer,
        time::{FormatTime, UtcTime},
        FmtContext, FormatEvent, FormatFields, Layer,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    EnvFilter, FmtSubscriber,
};

pub fn initialize() -> WorkerGuard {
    let formatter = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

    let file_appender = rolling::daily("./logs", "bathbot.log");
    let (file_writer, guard) = NonBlocking::new(file_appender);

    let file_layer = Layer::default()
        .event_format(FileEventFormat::new(formatter))
        .with_writer(file_writer);

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .with_timer(UtcTime::new(formatter))
        .finish()
        .with(file_layer);

    tracing::subscriber::set_global_default(subscriber).expect("failed to set global subscriber");

    guard
}

struct FileEventFormat<'f> {
    timer: UtcTime<&'f [FormatItem<'f>]>,
}

impl<'f> FileEventFormat<'f> {
    fn new(formatter: &'f [FormatItem<'f>]) -> Self {
        Self {
            timer: UtcTime::new(formatter),
        }
    }
}

impl<S, N> FormatEvent<S, N> for FileEventFormat<'_>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        self.timer.format_time(&mut writer)?;
        let metadata = event.metadata();

        write!(
            writer,
            " {:>5} [{}:{}] ",
            metadata.level(),
            metadata.file().unwrap_or_else(|| metadata.target()),
            metadata.line().unwrap_or(0),
        )?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}
