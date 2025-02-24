use std::fmt::Result as FmtResult;

use bathbot_util::datetime::NAIVE_DATETIME_FORMAT;
use time::format_description::FormatItem;
use tracing::{Event, Subscriber, level_filters::LevelFilter};
use tracing_appender::{
    non_blocking::{NonBlocking, WorkerGuard},
    rolling,
};
use tracing_subscriber::{
    EnvFilter, Layer as _,
    filter::Targets,
    fmt::{
        FmtContext, FormatEvent, FormatFields, Layer,
        format::Writer,
        time::{FormatTime, UtcTime},
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
};

pub fn init() -> Box<[WorkerGuard]> {
    let stdout_filter: EnvFilter = "bathbot=debug,sqlx=warn,tracking=off,info".parse().unwrap();

    let stdout_layer = Layer::default()
        .event_format(StdoutEventFormat::default())
        .with_filter(stdout_filter);

    let file_appender = rolling::daily("./logs", "bathbot.log");
    let (file_writer, file_guard) = NonBlocking::new(file_appender);

    let file_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => "bathbot=trace,info".parse().unwrap(),
    };

    let file_layer = Layer::default()
        .event_format(FileEventFormat::<true>::default())
        .with_writer(file_writer)
        .with_filter(file_filter.add_directive("tracking=off".parse().unwrap()));

    let tracking_appender = rolling::daily("./logs", "tracking.log");
    let (tracking_writer, tracking_guard) = NonBlocking::new(tracking_appender);

    let tracking_filter = Targets::new().with_target("tracking", LevelFilter::INFO);

    let tracking_layer = Layer::default()
        .event_format(FileEventFormat::<false>::default())
        .with_writer(tracking_writer)
        .with_filter(tracking_filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .with(tracking_layer)
        .init();

    let default_panic_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        // First log the panic
        let payload = panic_info.payload();

        let payload = if let Some(s) = payload.downcast_ref::<&str>() {
            Some(&**s)
        } else {
            payload.downcast_ref::<String>().map(String::as_str)
        };

        let location = panic_info.location().map(|l| l.to_string());

        error!(payload, location, "A panic occurred");

        // Then call the default panic handler
        default_panic_hook(panic_info);
    }));

    vec![file_guard, tracking_guard].into_boxed_slice()
}

struct StdoutEventFormat {
    timer: UtcTime<&'static [FormatItem<'static>]>,
}

impl Default for StdoutEventFormat {
    fn default() -> Self {
        Self {
            timer: UtcTime::new(NAIVE_DATETIME_FORMAT),
        }
    }
}

impl<S, N> FormatEvent<S, N> for StdoutEventFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> FmtResult {
        self.timer.format_time(&mut writer)?;
        let metadata = event.metadata();

        write!(writer, " {:>5} ", metadata.level(),)?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

struct FileEventFormat<const WITH_FILE: bool> {
    timer: UtcTime<&'static [FormatItem<'static>]>,
}

impl<const WITH_FILE: bool> Default for FileEventFormat<WITH_FILE> {
    fn default() -> Self {
        Self {
            timer: UtcTime::new(NAIVE_DATETIME_FORMAT),
        }
    }
}

impl<S, N, const WITH_FILE: bool> FormatEvent<S, N> for FileEventFormat<WITH_FILE>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> FmtResult {
        self.timer.format_time(&mut writer)?;
        let metadata = event.metadata();

        write!(writer, " {:>5} ", metadata.level())?;

        if WITH_FILE {
            match (metadata.file(), metadata.line()) {
                (Some(file), Some(line)) => write!(writer, "[{file}:{line}] ")?,
                (Some(file), None) => write!(writer, "[{file}:?] ")?,
                (None, Some(line)) => write!(writer, "[?:{line}] ")?,
                (None, None) => {}
            }
        }

        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}
