use core::str::FromStr;
use eyre::ContextCompat;
use lazy_static::lazy_static;
use regex::Regex;
use rev_lines::RevLines;
use tracing::warn;

// Define a struct to hold the parts of the log entry

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub datetime: i64,
    pub level: String,
    pub thread: String,
    pub path: String,
    pub line_number: usize,
    pub message: String,
}

lazy_static! {
    static ref CONTROL_SEQUENCE_PATTERN: Regex = Regex::new("\x1b\\[[0-9;]*m").unwrap();
}

impl FromStr for LogEntry {
    type Err = eyre::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        println!("before: {:?}", s);
        let no_control_sequence = CONTROL_SEQUENCE_PATTERN.replace_all(s, "");
        println!("after: {:?}", no_control_sequence);
        let mut split = no_control_sequence.split_whitespace();
        let datetime = split
            .next()
            .and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok())
            .context("no datetime")?;
        let level: tracing::Level = split.next().and_then(|x| x.parse().ok()).context("no level")?;
        let thread = split.next().context("no thread")?;
        let path = split.next().map(|x| x[..x.len() - 1].to_string()).context("no path")?;

        let line_number = split.next().and_then(|x| x[..x.len() - 1].parse().ok()).unwrap_or(0);
        let message = split.collect::<Vec<&str>>().join(" ");
        Ok(LogEntry {
            datetime: datetime.timestamp_millis(),
            level: level.to_string(),
            thread: thread.to_string(),
            path,
            line_number,
            message,
        })
    }
}

pub async fn get_log_entries(path: impl AsRef<std::path::Path>, limit: usize) -> eyre::Result<Vec<LogEntry>> {
    // Specify the path to your log file
    let file = std::fs::File::open(path.as_ref())?;
    let mut lines = RevLines::new(file);
    // get all entries first
    let mut entries = tokio::task::spawn_blocking(move || {
        let mut entries = vec![];
        while let Some(line) = lines.next() {
            if entries.len() >= limit {
                break;
            }
            let line = match line {
                Ok(line) => line,
                Err(error) => {
                    warn!("Error reading line: {:?}", error);
                    entries.push(LogEntry {
                        datetime: 0,
                        level: "".to_string(),
                        thread: "".to_string(),
                        path: "".to_string(),
                        line_number: 0,
                        message: error.to_string(),
                    });
                    break;
                }
            };
            let entry = LogEntry::from_str(&line);
            match entry {
                Ok(entry) => entries.push(entry),
                Err(_) => entries.push(LogEntry {
                    datetime: 0,
                    level: "".to_string(),
                    thread: "".to_string(),
                    path: "".to_string(),
                    line_number: 0,
                    message: line,
                }),
            }
        }
        entries
    })
    .await?;
    if entries.is_empty() {
        entries.push(LogEntry {
            datetime: 0,
            level: "".to_string(),
            thread: "".to_string(),
            path: "".to_string(),
            line_number: 0,
            message: format!("No entries found in {}", path.as_ref().display()),
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_entry_from_line() {
        let line = "2024-05-18T14:26:36.709390Z  WARN                 main trading_be: 290: terminated diff 0_table_limit_1 initially";
        let entry = LogEntry::from_str(line).unwrap();
        // assert_eq!(entry.datetime, "2024-02-09 18:10:46");
        assert_eq!(entry.level, "WARN");
        assert_eq!(entry.thread, "main");
        assert_eq!(entry.path, "trading_be");
        assert_eq!(entry.line_number, 290);
        assert_eq!(entry.message, "terminated diff 0_table_limit_1 initially");
    }
    #[test]
    fn log_entry_from_line_control_sequence() {
        let line = "\u{1b}[2m2024-06-07T12:25:06.735143Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m main \u{1b}[2mtrading_be::strategy::data_factory\u{1b}[0m\u{1b}[2m:\u{1b}[0m \u{1b}[2m110:\u{1b}[0m terminated diff 0_table_limit_1 initially";
        let entry = LogEntry::from_str(line).unwrap();
        // assert_eq!(entry.datetime, "2024-02-09 18:10:46");
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.thread, "main");
        assert_eq!(entry.path, "trading_be::strategy::data_factory");
        assert_eq!(entry.line_number, 110);
        assert_eq!(entry.message, "terminated diff 0_table_limit_1 initially");
    }
    #[tokio::test]
    // #[allow(dead_code)]
    async fn log_entry_from_file() {
        // NOTE use some valid file when needed
        let path = "/Users/jack/Dev/InsolventCapital/trading.insolvent.app-backend/log/user.log";
        let entries = get_log_entries(path, 20).await.unwrap();
        println!("{:?}", entries[0]);
        assert_eq!(entries.len(), 20);
    }
}
