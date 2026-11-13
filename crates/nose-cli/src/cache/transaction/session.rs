//! A watch session retains analysis in memory, not a permanent disk lease.
use super::*;

pub(crate) struct SessionBudget {
    stored_bytes: u64,
    written_at_check: u64,
}

impl SessionBudget {
    pub(crate) fn new(run: &CacheRun) -> Self {
        if let Err(error) = run.commit() {
            eprintln!("warning: cache commit failed: {error}");
        }
        Self {
            stored_bytes: super::super::admin::status(run.root(), run.max_bytes()).bytes,
            written_at_check: run.written_bytes(),
        }
    }

    pub(crate) fn finish_revision(&mut self, run: &mut CacheRun) {
        if let Err(error) = run.commit() {
            eprintln!("warning: cache commit failed: {error}");
        }
        // All borrowed line contexts and background publications have finished.
        // Immutable features are already in memory; eviction is now a cache miss
        // on a future revision, never a dangling live artifact read.
        run._store_lease.take();
        let added = run.written_bytes().saturating_sub(self.written_at_check);
        if self.stored_bytes.saturating_add(added) >= run.max_bytes() {
            if let Err(error) = super::super::admin::try_prune(run.root(), run.max_bytes()) {
                if std::env::var_os("NOSE_CACHE_STATS").is_some() {
                    eprintln!("  [cache-prune] skipped: {error}");
                }
            }
            self.stored_bytes = super::super::admin::status(run.root(), run.max_bytes()).bytes;
            self.written_at_check = run.written_bytes();
        }
    }
}

impl CacheRun {
    pub(crate) fn begin_session_revision(&mut self) {
        if self._store_lease.is_none() {
            self._store_lease = shared_store_lease(&self.root).map(Arc::new);
        }
    }
}
