use crate::{BotResult, Error};

use flexi_logger::{
    writers::FileLogWriter, Age, Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, Logger,
    LoggerHandle, Naming,
};
use log::Record;
use once_cell::sync::OnceCell;
use std::io::{Result as IoResult, Write};

static LOGGER: OnceCell<LoggerHandle> = OnceCell::new();

pub fn initialize() -> BotResult<()> {
    let file_spec = FileSpec::default().directory("logs");
    let tracking_file_spec = FileSpec::default().directory("logs/tracking/");

    let tracking_log_writer = FileLogWriter::builder(tracking_file_spec)
        .format(log_format_files)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogAndCompressedFiles(5, 20),
        )
        .try_build()
        .expect("failed to build tracking_log_writer");

    let logger_handle = Logger::try_with_str("bathbot_twilight")
        .unwrap()
        .log_to_file(file_spec)
        .add_writer("tracking", Box::new(tracking_log_writer))
        .format(log_format)
        .format_for_files(log_format_files)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogAndCompressedFiles(5, 20),
        )
        .duplicate_to_stdout(Duplicate::Info)
        .start_with_specfile("logconfig.toml")
        .map_err(|_| Error::NoLoggingSpec)?;

    let _ = LOGGER.set(logger_handle);

    Ok(())
}

pub fn log_format(w: &mut dyn Write, now: &mut DeferredNow, record: &Record) -> IoResult<()> {
    write!(
        w,
        "[{}] {} {}",
        now.now().format("%y-%m-%d %H:%M:%S"),
        record.level(),
        &record.args()
    )
}

pub fn log_format_files(w: &mut dyn Write, now: &mut DeferredNow, record: &Record) -> IoResult<()> {
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
