use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use eyre::eyre;
use serde::{Deserialize, Serialize};
use tracing::{level_filters::LevelFilter, Level};
use tracing_subscriber::EnvFilter;

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[default]
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
    Detail,
}

impl From<LogLevel> for LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Detail => LevelFilter::TRACE,
            LogLevel::Off => LevelFilter::OFF,
        }
    }
}

impl From<LogLevel> for Level {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
            LogLevel::Off => Level::TRACE,
            LogLevel::Detail => Level::TRACE,
        }
    }
}

impl FromStr for LogLevel {
    type Err = eyre::Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_ref() {
            "error" => Ok(LogLevel::Error),
            "warn" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            "detail" => Ok(LogLevel::Detail),
            "off" => Ok(LogLevel::Off),
            _ => Err(eyre!("Invalid log level: {}", s)),
        }
    }
}

fn build_env_filter(log_level: LogLevel) -> eyre::Result<EnvFilter> {
    let level: Level = log_level.into();
    let mut filter = EnvFilter::from_default_env().add_directive(level.into());
    if log_level != LogLevel::Detail {
        filter = filter
            .add_directive("tungstenite::protocol=debug".parse()?)
            .add_directive("tokio_postgres::connection=debug".parse()?)
            .add_directive("tokio_util::codec::framed_impl=debug".parse()?)
            .add_directive("tokio_tungstenite=debug".parse()?)
            .add_directive("h2=info".parse()?)
            .add_directive("rustls::client::hs=info".parse()?)
            .add_directive("rustls::client::tls13=info".parse()?)
            .add_directive("hyper::client=info".parse()?)
            .add_directive("hyper::proto=info".parse()?)
            .add_directive("mio=info".parse()?)
            .add_directive("want=info".parse()?)
            .add_directive("sqlparser=info".parse()?);
    }
    Ok(filter)
}

pub enum LoggingGuard {
    NonBlocking(tracing_appender::non_blocking::WorkerGuard, PathBuf),
    StdoutWithPath(Option<PathBuf>),
}
impl LoggingGuard {
    pub fn get_file(&self) -> Option<PathBuf> {
        match self {
            LoggingGuard::NonBlocking(_guard, path) => Some(path.clone()),
            LoggingGuard::StdoutWithPath(path) => path.clone(),
        }
    }
}
pub fn setup_logs(log_level: LogLevel, _file: Option<PathBuf>) -> eyre::Result<LoggingGuard> {
    let filter = build_env_filter(log_level)?;

    let fmt = tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_line_number(true)
        .with_env_filter(filter);
    // let guard =
    // if let Some(path) = file {
    //     let file = OpenOptions::new()
    //         .append(true)
    //         .open(&path)
    //         .with_context(|| format!("Failed to open log file: {}", path.display()))?;
    //     let (non_blocking, guard) = tracing_appender::non_blocking(file);
    //
    //     fmt.with_writer(non_blocking).init();
    //     LoggingGuard::NonBlocking(guard, path)
    // } else {
    fmt.with_writer(std::io::stdout).init();
    let guard = LoggingGuard::StdoutWithPath(_file);
    // };
    log_panics::init();
    Ok(guard)
}

#[derive(Clone)]
pub struct DynLogger {
    logger: Arc<dyn Fn(&str) + Send + Sync>,
}
impl DynLogger {
    pub fn new(logger: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        Self { logger }
    }
    pub fn empty() -> Self {
        Self {
            logger: Arc::new(|_| {}),
        }
    }
    pub fn log(&self, msg: impl AsRef<str>) {
        (self.logger)(msg.as_ref())
    }
}

/// actually test writing, there is no direct way to check if the application has the ownership or the write access
pub fn can_create_file_in_directory(directory: &str) -> bool {
    let test_file_path: String = format!("{}/test_file.txt", directory);
    match std::fs::File::create(&test_file_path) {
        Ok(file) => {
            // File created successfully; remove it after checking
            drop(file);
            if let Err(err) = std::fs::remove_file(&test_file_path) {
                eprintln!("Error deleting test file: {}", err);
            }
            true
        }
        Err(_) => false,
    }
}
