//! String interning. Identifiers, field names, and canonical-builtin names are
//! interned to a single [`Symbol`] (a 4-byte `Copy` key) so that name equality
//! across files is a cheap integer compare — important for detection at scale.

use lasso::{Key, Spur, ThreadedRodeo};
use std::sync::Arc;

/// Interned string key. Cheap to copy and compare.
pub type Symbol = Spur;

/// A stable integer index for a [`Symbol`] within one interner. NOTE: ids are
/// assigned in interning order, which is nondeterministic under parallel
/// lowering — do NOT use this in any hash that must be reproducible across runs.
/// Use [`Interner::symbol_hash`] for that.
pub fn symbol_index(s: Symbol) -> u32 {
    s.into_usize() as u32
}

/// A shared, thread-safe string interner.
///
/// Cloning is cheap (`Arc` bump) and clones share the same backing store, so a
/// `Symbol` produced on one worker thread is valid and comparable everywhere.
#[derive(Clone, Default)]
pub struct Interner {
    inner: Arc<ThreadedRodeo>,
}

impl Interner {
    pub fn new() -> Self {
        Interner {
            inner: Arc::new(ThreadedRodeo::new()),
        }
    }

    /// Intern `s`, returning its stable [`Symbol`].
    pub fn intern(&self, s: &str) -> Symbol {
        self.inner.get_or_intern(s)
    }

    /// Resolve a [`Symbol`] back to its string. Panics if the symbol came from a
    /// different interner (a programming error).
    pub fn resolve(&self, sym: Symbol) -> &str {
        self.inner.resolve(&sym)
    }

    /// A content hash of a symbol's string (FNV-1a). Stable across runs — unlike
    /// the interner-assigned id — so it is safe to use in reproducible
    /// fingerprints even though lowering interns in parallel.
    pub fn symbol_hash(&self, sym: Symbol) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.resolve(sym).bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}
