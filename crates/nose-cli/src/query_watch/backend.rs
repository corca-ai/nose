use anyhow::{Context, Result};
use notify::{Config, Event, PollWatcher, Watcher};
use std::sync::mpsc::Sender;
use std::time::Duration;

const POLL_INTERVAL: &str = "NOSE_WATCH_POLL_INTERVAL_MS";

pub(super) fn new(sender: Sender<notify::Result<Event>>) -> Result<Box<dyn Watcher>> {
    match std::env::var(POLL_INTERVAL) {
        Ok(value) => {
            let interval = poll_interval(&value)?;
            let config = Config::default()
                .with_poll_interval(interval)
                .with_compare_contents(true);
            Ok(Box::new(PollWatcher::new(sender, config)?))
        }
        Err(std::env::VarError::NotPresent) => Ok(Box::new(notify::recommended_watcher(sender)?)),
        Err(error) => Err(error).with_context(|| format!("reading {POLL_INTERVAL}")),
    }
}

fn poll_interval(value: &str) -> Result<Duration> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .with_context(|| format!("{POLL_INTERVAL} must be a positive integer in milliseconds"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polling_configuration_requires_a_positive_millisecond_interval() {
        assert_eq!(poll_interval("100").unwrap(), Duration::from_millis(100));
        for invalid in ["", "0", "-1", "0.5", "100ms"] {
            assert!(poll_interval(invalid).is_err(), "{invalid}");
        }
    }
}
