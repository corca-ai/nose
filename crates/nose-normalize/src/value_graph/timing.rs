use super::*;

pub(super) struct ValueGraphBuildTimer<'a> {
    enabled: bool,
    il: &'a Il,
    root: NodeId,
    total_start: Option<std::time::Instant>,
    stage_start: Option<std::time::Instant>,
    seed_ms: f64,
    immutable_ms: f64,
    inline_ms: f64,
    process_ms: f64,
}

impl<'a> ValueGraphBuildTimer<'a> {
    pub(super) fn new(il: &'a Il, root: NodeId) -> Self {
        let enabled = std::env::var_os("NOSE_TIME_VALUE_GRAPH").is_some();
        let now = enabled.then(std::time::Instant::now);
        Self {
            enabled,
            il,
            root,
            total_start: now,
            stage_start: now,
            seed_ms: 0.0,
            immutable_ms: 0.0,
            inline_ms: 0.0,
            process_ms: 0.0,
        }
    }

    fn take_stage(&mut self) -> f64 {
        let Some(start) = self.stage_start else {
            return 0.0;
        };
        let elapsed = start.elapsed().as_secs_f64() * 1e3;
        self.stage_start = Some(std::time::Instant::now());
        elapsed
    }

    pub(super) fn mark_seed(&mut self) {
        self.seed_ms = self.take_stage();
    }

    pub(super) fn mark_immutable(&mut self) {
        self.immutable_ms = self.take_stage();
    }

    pub(super) fn mark_inline(&mut self) {
        self.inline_ms = self.take_stage();
    }

    pub(super) fn mark_process(&mut self) {
        self.process_ms = self.take_stage();
    }

    pub(super) fn mark_finish(&mut self, value_nodes: usize, sinks: usize) {
        if !self.enabled {
            return;
        }
        let finish_ms = self.take_stage();
        let total_ms = self
            .total_start
            .map(|start| start.elapsed().as_secs_f64() * 1e3)
            .unwrap_or(0.0);
        let span = self.il.node(self.root).span;
        eprintln!(
            "  [value-graph] {:?} {}:{}-{} total={total_ms:.1}ms seed={:.1}ms immutable={:.1}ms inline={:.1}ms process={:.1}ms finish={finish_ms:.1}ms nodes={} sinks={}",
            self.il.kind(self.root),
            self.il.meta.path,
            span.start_line,
            span.end_line,
            self.seed_ms,
            self.immutable_ms,
            self.inline_ms,
            self.process_ms,
            value_nodes,
            sinks,
        );
    }
}
