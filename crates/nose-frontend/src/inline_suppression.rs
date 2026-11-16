//! Source-unit admission policy shared by live and cached analysis.
use nose_il::Il;
use serde::{Deserialize, Serialize};

/// Whether inline `nose-ignore` markers remove units before detection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InlineSuppression {
    /// Preserve the ordinary frontend behavior.
    #[default]
    Respect,
    /// Include marked units and their contiguous token streams.
    Include,
}

impl InlineSuppression {
    /// Stable name used by analysis profiles and cache identities.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Respect => "respect",
            Self::Include => "include",
        }
    }

    pub(crate) fn apply(self, il: &mut Il) {
        if il.suppressed.is_empty() {
            return;
        }
        let contents = il.edit();
        match self {
            Self::Respect => {
                let spans: rustc_hash::FxHashSet<_> = contents.suppressed.iter().copied().collect();
                contents.units.retain(|unit| {
                    let span = contents.nodes[unit.root.0 as usize].span;
                    !spans.contains(&(span.start_byte, span.end_byte))
                });
            }
            Self::Include => contents.suppressed.clear(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, Interner, Lang};

    #[test]
    fn policy_preserves_marked_units_in_plain_and_embedded_sources() {
        for (lang, path, source) in [
            (Lang::Python, "a.py", "# NoSe-IgNoRe\n@memoize\n@trace\ndef f(x):\n    return x + 1\n"),
            (Lang::Vue, "a.vue", "<script>\n// nose-ignore\nfunction f(x) { return x + 1; }\n</script>\n<style>\n/* nose-ignore */\n.a { color: red; }\n</style>\n<template><div>hello</div></template>"),
        ] {
            let interner = Interner::new();
            let lower = |policy| crate::try_lower_source_regions_with_policy(FileId(0), path, source.as_bytes(), lang, &interner, policy).unwrap();
            let respected = lower(InlineSuppression::Respect);
            let included = lower(InlineSuppression::Include);
            assert_eq!(respected.len(), included.len());
            assert!(respected.iter().any(|il| !il.suppressed.is_empty()));
            for (respect, include) in respected.iter().zip(&included) {
                assert!(include.suppressed.is_empty());
                assert_eq!(include.units.len(), respect.units.len() + respect.suppressed.len());
                assert_eq!(include.source.as_ref().unwrap().digest(), nose_il::ContentDigest::sha256(source.as_bytes()));
            }
        }
    }
}
