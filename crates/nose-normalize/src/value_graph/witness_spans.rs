//! Occurrence locations for witness export, independent of hash-consed creation spans.
use super::*;

pub(super) struct WitnessSpans {
    root: Span,
    occurrences: FxHashMap<ValueId, Option<Span>>,
}
impl WitnessSpans {
    pub(super) fn new(root: Span) -> Self {
        Self {
            root,
            occurrences: FxHashMap::default(),
        }
    }
    pub(super) fn observe(&mut self, value: ValueId, span: Span) {
        if span.file != self.root.file
            || span.start_byte < self.root.start_byte
            || span.end_byte > self.root.end_byte
            || span.start_byte >= span.end_byte
        {
            return;
        }
        self.occurrences
            .entry(value)
            .and_modify(|old| {
                if let Some(previous) = old {
                    // Wrappers can evaluate to their operand: retain the inner occurrence.
                    if previous.start_byte <= span.start_byte && span.end_byte <= previous.end_byte
                    {
                        *previous = span;
                    } else if !(span.start_byte <= previous.start_byte
                        && previous.end_byte <= span.end_byte)
                    {
                        // Equal values at different sites have no unique source address.
                        *old = None;
                    }
                }
            })
            .or_insert(Some(span));
    }
    pub(super) fn lines(&self, value: ValueId) -> (u32, u32) {
        self.occurrences
            .get(&value)
            .copied()
            .flatten()
            .map_or((0, 0), |span| (span.start_line, span.end_line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_and_foreign_occurrences_do_not_claim_a_source_location() {
        let file = nose_il::FileId(0);
        let mut spans = WitnessSpans::new(Span::new(file, 10, 100, 2, 10));
        spans.observe(0, Span::new(file, 20, 25, 3, 3));
        spans.observe(0, Span::new(file, 50, 55, 6, 6));
        spans.observe(0, Span::new(file, 20, 25, 3, 3));
        assert_eq!(spans.lines(0), (0, 0));
        spans.observe(1, Span::new(nose_il::FileId(1), 20, 25, 3, 3));
        assert_eq!(spans.lines(1), (0, 0));
        spans.observe(2, Span::new(file, 20, 40, 3, 4));
        spans.observe(2, Span::new(file, 25, 30, 3, 3));
        assert_eq!(spans.lines(2), (3, 3));
    }
}
