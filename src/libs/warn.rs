use std::collections::HashMap;
use tracing::warn;

#[derive(Debug)]
pub struct WarnStats {
    pub warn_count: u64,
}
#[derive(Debug, Default)]
pub struct WarnManager {
    warns: HashMap<String, WarnStats>,
}

impl WarnManager {
    pub fn new() -> Self {
        Self { warns: HashMap::new() }
    }

    pub fn warn(&mut self, s: impl AsRef<str>) {
        let s = s.as_ref();

        if let Some(stats) = self.warns.get_mut(s) {
            stats.warn_count += 1;
        } else {
            warn!("First warning: {}", s);
            self.warns.insert(s.to_string(), WarnStats { warn_count: 1 });
        }
    }
    pub fn get_warns(&self) -> &HashMap<String, WarnStats> {
        &self.warns
    }

    pub fn dump_stats(&self, mut file: impl std::io::Write) -> std::io::Result<()> {
        writeln!(file, "Warnings at {}:", chrono::Utc::now())?;
        for (msg, stats) in &self.warns {
            writeln!(file, "count={} warn={}", stats.warn_count, msg)?;
        }
        Ok(())
    }
}
