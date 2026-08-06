/// Per-stage wall-clock timing, printed to stderr when `NOSE_TIME` is set.
pub(super) struct StageTimer {
    enabled: bool,
    start: std::time::Instant,
    last: std::time::Instant,
}

impl StageTimer {
    pub(super) fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            enabled: std::env::var_os("NOSE_TIME").is_some(),
            start: now,
            last: now,
        }
    }

    pub(super) fn lap(&mut self, stage: &str) {
        let now = std::time::Instant::now();
        if self.enabled {
            eprintln!(
                "  [time] {stage:<12} {:>7.1}ms   (total {:>7.1}ms)",
                now.duration_since(self.last).as_secs_f64() * 1e3,
                now.duration_since(self.start).as_secs_f64() * 1e3,
            );
        }
        self.last = now;
    }
}
