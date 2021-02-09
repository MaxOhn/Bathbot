use crate::Error;

use flexi_logger::{Age, Cleanup, Criterion, DeferredNow, Duplicate, Logger, LoggerHandle, Naming};
use log::Record;
use once_cell::sync::OnceCell;

static LOGGER: OnceCell<LoggerHandle> = OnceCell::new();

pub fn initialize() -> Result<(), Error> {
    let log_init_status = LOGGER.set(
        Logger::with_str("bathbot_twilight")
            .log_to_file()
            .directory("logs")
            .format(log_format)
            .format_for_files(log_format_files)
            .o_timestamp(true)
            .rotate(
                Criterion::Age(Age::Day),
                Naming::Timestamps,
                Cleanup::KeepLogAndCompressedFiles(10, 20),
            )
            .duplicate_to_stdout(Duplicate::Info)
            .start_with_specfile("logconfig.toml")
            .map_err(|_| Error::NoLoggingSpec)?,
    );

    if log_init_status.is_err() {
        error!("LOGGER was already set");
    }

    Ok(())
}

pub fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {} {}",
        now.now().format("%y-%m-%d %H:%M:%S"),
        record.level(),
        &record.args()
    )
}

pub fn log_format_files(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {:^5} [{}:{}] {}",
        now.now().format("%y-%m-%d %H:%M:%S"),
        record.level(),
        record.file_static().unwrap_or_else(|| record.target()),
        record.line().unwrap_or(0),
        &record.args()
    )
}
