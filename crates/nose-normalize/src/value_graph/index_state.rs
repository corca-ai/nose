//! Same-unit indexed-element read-forwarding for value-graph construction (#337).
//!
//! Unlike field state (`field_state.rs`), which is keyed by a non-aliasable `this`/`self`
//! receiver and flushed as order-INSENSITIVE place sinks, array indices CAN alias (two
//! distinct value nodes `i` and `j` may be equal at runtime), so a place-keyed, order-
//! insensitive model is UNSOUND for `a[i]`. The element WRITE therefore stays an ordered
//! effect (`push_effect`, order-sensitive — already sound). This module adds only READ-
//! FORWARDING: a `base[index]` read evaluated AFTER a write to that exact (base, index) place
//! returns the written value, so `clobber` (`a[i]=a[j]; a[j]=a[i]`) — whose second read sees
//! the first write — fingerprints distinctly from `swap` (`t=a[i]; a[i]=a[j]; a[j]=t`).
//!
//! Soundness under aliasing is conservative: ANY indexed write clears the entire forwarding
//! map before recording the just-written place, so a read only forwards when its place was
//! the most recent write with no intervening (possibly-aliasing) write. This errs toward
//! FEWER forwards (more distinct nodes), which can only cost recall, never soundness.
//!
//! A Lean obligation analogous to `normalize.value_graph.field_writes` (last-write-wins, write
//! order matters) is a tracked follow-up; for now the argument is conservative-by-construction
//! (the gates above) plus the `nose verify` soundness oracle, which interprets in-place element
//! mutation in lockstep (`interp.rs` `bind` for `Index`) and would witness any false merge.

use super::*;

/// A forwardable indexed place: the base value and the index value (both `ValueId`s in the
/// value graph). Two reads share a place only when base AND index are the SAME value node —
/// distinct nodes are treated as possibly-aliasing and never forwarded across.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct IndexStateKey {
    pub(super) base: ValueId,
    pub(super) index: ValueId,
}

impl Builder<'_> {
    /// Record an indexed element write `base[index] = value` for read-forwarding. The write
    /// itself is captured separately as an ordered effect; this only updates what a LATER
    /// read of the same place sees. Conservative on two axes:
    /// - clear all prior forwards first (any write may alias any outstanding place);
    /// - only INSTALL a forward for an UNCONDITIONAL (top-level, `path` empty) write. A write
    ///   under a branch condition would otherwise forward unconditionally to a later read
    ///   (`if c { a[i]=w } ; a[i]` must not become `w`), so a conditional write only
    ///   invalidates — never forwards. Combined with `push_effect` clearing `index_env`, this
    ///   confines forwarding to straight-line, effect-free runs, which is sound by construction.
    pub(super) fn record_index_write(&mut self, base: ValueId, index: ValueId, value: ValueId) {
        self.index_env.clear();
        // Only install a forward for an UNCONDITIONAL, top-level write — `path` empty (not under
        // a branch) and `loop_depth == 0` (not inside a loop body, where the "last write" is an
        // iteration-relative symbolic value, not the concrete post-loop element). Either way the
        // write itself is still recorded as an ordered effect; only the forward is suppressed.
        if self.path.is_empty() && self.loop_depth == 0 {
            self.index_env.insert(IndexStateKey { base, index }, value);
        }
    }

    /// The forwarded value for a `base[index]` read, if that exact place was the most recent
    /// indexed write (see `record_index_write`). `None` → no forward; emit a fresh `Index`.
    pub(super) fn forwarded_index_read(&self, base: ValueId, index: ValueId) -> Option<ValueId> {
        self.index_env
            .get(&IndexStateKey { base, index })
            .copied()
    }
}
