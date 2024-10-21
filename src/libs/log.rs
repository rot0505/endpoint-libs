use std::{fs::OpenOptions, path::PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use eyre::eyre;
use eyre::WrapErr;
use serde::{Deserialize, Serialize};
use tracing::{level_filters::LevelFilter, Level};
use tracing_subscriber::fmt::{self, layer};
use tracing_subscriber::{registry, EnvFilter};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::Layer;
use tracing_subscriber::util::SubscriberInitExt;

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
pub fn setup_logs(log_level: LogLevel, log_dir_and_file_prefix: Option<(PathBuf, &str)>) -> eyre::Result<()> {
    let filter = build_env_filter(log_level)?;

    let stdout_layer: tracing_subscriber::filter::Filtered<fmt::Layer<registry::Registry>, EnvFilter, registry::Registry> = fmt::layer()
    .with_thread_names(true)
    .with_line_number(true)
    .with_filter(filter);
    
    if let Some((log_dir, file_prefix)) = log_dir_and_file_prefix {
        let file_filter = build_env_filter(log_level)?;
        registry()
        .with(stdout_layer)
        .with(fmt::layer()
        .with_thread_names(true)
        .with_line_number(true)
        .with_writer(tracing_appender::rolling::hourly(log_dir, file_prefix))
        .with_filter(file_filter))
        .init();
    } else {
        registry()
        .with(stdout_layer)
        .init();
    }

    Ok(())
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
