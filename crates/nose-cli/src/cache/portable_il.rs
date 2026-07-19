// #873 freezes the raw/resolved codec before #874 activates dependency-aware
// loading. Keeping the implementation in normal builds makes that boundary a
// real product format rather than test-only code.
#![allow(dead_code)]

use super::digest::{ContentDigest, StableSha256};
use anyhow::{bail, Context, Result};
use nose_il::{
    symbol_index, CallTargetEvidenceKind, EvidenceAnchor, EvidenceKind, EvidenceRecord, FileId,
    GuardEvidenceKind, Il, Interner, Payload, PromiseSettledValueEvidenceKind, Span,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

const PORTABLE_IL_SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
struct PortableIl {
    schema: u32,
    region_id: String,
    /// Lexicographically sorted. The serialized `Spur` ids in `il` index this
    /// table, never a process-local interner.
    symbols: Vec<String>,
    il: Il,
}

/// Path-independent identity of one discovered source snapshot.
pub(super) fn source_digest(lang: nose_il::Lang, source: &[u8]) -> ContentDigest {
    ContentDigest::derive(
        b"nose.source-snapshot.v1",
        &[lang.name().as_bytes(), source],
    )
}

/// Full semantic/reporting identity of an IL, excluding only checkout-local path
/// and process-local FileId/interner ids. Evidence records are hashed in full.
pub(super) fn semantic_digest(il: &Il, interner: &Interner) -> ContentDigest {
    let mut hasher = StableSha256::new(b"nose.portable-il.semantic.v1");
    il.meta.lang.hash(&mut hasher);
    il.root.hash(&mut hasher);
    hash_len(&mut hasher, il.nodes.len());
    for node in &il.nodes {
        node.kind.hash(&mut hasher);
        match node.payload {
            Payload::Name(symbol) => {
                1_u8.hash(&mut hasher);
                interner.resolve(symbol).hash(&mut hasher);
            }
            payload => {
                0_u8.hash(&mut hasher);
                payload.hash(&mut hasher);
            }
        }
        hash_span(&mut hasher, node.span);
        node.child_start.hash(&mut hasher);
        node.child_len.hash(&mut hasher);
    }
    il.edges.hash(&mut hasher);
    hash_len(&mut hasher, il.units.len());
    for unit in &il.units {
        unit.root.hash(&mut hasher);
        unit.kind.hash(&mut hasher);
        match unit.name {
            Some(symbol) => {
                1_u8.hash(&mut hasher);
                interner.resolve(symbol).hash(&mut hasher);
            }
            None => 0_u8.hash(&mut hasher),
        }
        unit.origin.hash(&mut hasher);
    }
    hash_len(&mut hasher, il.cid_names.len());
    for &symbol in &il.cid_names {
        interner.resolve(symbol).hash(&mut hasher);
    }
    il.suppressed.hash(&mut hasher);
    hash_len(&mut hasher, il.evidence.len());
    for record in &il.evidence {
        canonical_evidence(record, FileId(0)).hash(&mut hasher);
    }
    hasher.finish_digest()
}

/// Serialize one raw or resolved IL with deterministic symbols and neutral
/// checkout provenance. The CAS stage envelope distinguishes raw from resolved.
pub(super) fn encode(il: &Il, interner: &Interner) -> Result<Vec<u8>> {
    let symbols = symbols(il, interner);
    let canonical_interner = Interner::new();
    for symbol in &symbols {
        canonical_interner.intern(symbol);
    }
    let mut canonical = il.clone();
    remap_symbols(&mut canonical, interner, &canonical_interner);
    rebind_file(&mut canonical, FileId(0), String::new());
    let portable = PortableIl {
        schema: PORTABLE_IL_SCHEMA,
        region_id: region_identity(&canonical).hex(),
        symbols,
        il: canonical,
    };
    serde_json::to_vec(&portable).context("serialize portable IL")
}

/// Restore an artifact into the caller's shared interner and current checkout.
/// Invalid symbol ids, schema drift, or region corruption fail closed.
pub(super) fn decode(bytes: &[u8], interner: &Interner, file: FileId, path: String) -> Result<Il> {
    let mut portable: PortableIl =
        serde_json::from_slice(bytes).context("deserialize portable IL")?;
    if portable.schema != PORTABLE_IL_SCHEMA {
        bail!(
            "portable IL schema {} is not supported (expected {})",
            portable.schema,
            PORTABLE_IL_SCHEMA
        );
    }
    if !portable.symbols.windows(2).all(|pair| pair[0] < pair[1]) {
        bail!("portable IL symbol table is not sorted and unique");
    }
    validate_symbol_ids(&portable.il, portable.symbols.len())?;
    if portable.region_id != region_identity(&portable.il).hex() {
        bail!("portable IL region identity does not match its payload");
    }

    let artifact_interner = Interner::new();
    for (index, symbol) in portable.symbols.iter().enumerate() {
        let actual = artifact_interner.intern(symbol);
        if symbol_index(actual) as usize != index {
            bail!("portable IL symbol table produced a non-canonical id");
        }
    }
    remap_symbols(&mut portable.il, &artifact_interner, interner);
    rebind_file(&mut portable.il, file, path);
    Ok(portable.il)
}

/// Stable identity for the script/style/markup region within one container.
/// It deliberately excludes source positions and contents, so an edit that moves
/// a region does not turn it into a different logical sub-file.
pub(super) fn region_identity(il: &Il) -> ContentDigest {
    let mut hasher = StableSha256::new(b"nose.portable-il.region.v1");
    il.meta.lang.hash(&mut hasher);
    il.node(il.root).kind.hash(&mut hasher);
    il.units
        .first()
        .map(|unit| unit.origin.container_kind)
        .hash(&mut hasher);
    hasher.finish_digest()
}

fn hash_len(hasher: &mut impl Hasher, len: usize) {
    hasher.write_u64(len as u64);
}

fn hash_span(hasher: &mut impl Hasher, span: Span) {
    // FileId is process-local. Every other coordinate affects query output.
    hasher.write_u32(span.start_byte);
    hasher.write_u32(span.end_byte);
    hasher.write_u32(span.start_line);
    hasher.write_u32(span.end_line);
}

fn symbols(il: &Il, interner: &Interner) -> Vec<String> {
    let mut symbols = BTreeSet::new();
    for node in &il.nodes {
        if let Payload::Name(symbol) = node.payload {
            symbols.insert(interner.resolve(symbol).to_owned());
        }
    }
    for unit in &il.units {
        if let Some(symbol) = unit.name {
            symbols.insert(interner.resolve(symbol).to_owned());
        }
    }
    for &symbol in &il.cid_names {
        symbols.insert(interner.resolve(symbol).to_owned());
    }
    symbols.into_iter().collect()
}

fn remap_symbols(il: &mut Il, source: &Interner, target: &Interner) {
    for node in &mut il.nodes {
        if let Payload::Name(symbol) = node.payload {
            node.payload = Payload::Name(target.intern(source.resolve(symbol)));
        }
    }
    for unit in &mut il.units {
        if let Some(symbol) = unit.name {
            unit.name = Some(target.intern(source.resolve(symbol)));
        }
    }
    for symbol in &mut il.cid_names {
        *symbol = target.intern(source.resolve(*symbol));
    }
}

fn validate_symbol_ids(il: &Il, symbols: usize) -> Result<()> {
    let valid = |symbol| (symbol_index(symbol) as usize) < symbols;
    if il
        .nodes
        .iter()
        .any(|node| matches!(node.payload, Payload::Name(symbol) if !valid(symbol)))
        || il
            .units
            .iter()
            .any(|unit| unit.name.is_some_and(|symbol| !valid(symbol)))
        || il.cid_names.iter().any(|&symbol| !valid(symbol))
    {
        bail!("portable IL references a symbol outside its table");
    }
    Ok(())
}

fn rebind_file(il: &mut Il, file: FileId, path: String) {
    il.file = file;
    il.meta.path = path;
    for node in &mut il.nodes {
        node.span.file = file;
    }
    for record in &mut il.evidence {
        *record = canonical_evidence(record, file);
    }
}

fn canonical_evidence(record: &EvidenceRecord, file: FileId) -> EvidenceRecord {
    let mut record = record.clone();
    record.anchor = rebind_anchor(record.anchor, file);
    record.kind = rebind_kind(record.kind, file);
    record
}

fn span_file(mut span: Span, file: FileId) -> Span {
    span.file = file;
    span
}

fn rebind_anchor(anchor: EvidenceAnchor, file: FileId) -> EvidenceAnchor {
    match anchor {
        EvidenceAnchor::SourceSpan(span) => EvidenceAnchor::SourceSpan(span_file(span, file)),
        EvidenceAnchor::Node { span, kind } => EvidenceAnchor::Node {
            span: span_file(span, file),
            kind,
        },
        EvidenceAnchor::Param { span } => EvidenceAnchor::Param {
            span: span_file(span, file),
        },
        EvidenceAnchor::Binding { span, local_hash } => EvidenceAnchor::Binding {
            span: span_file(span, file),
            local_hash,
        },
        EvidenceAnchor::Sequence { span } => EvidenceAnchor::Sequence {
            span: span_file(span, file),
        },
    }
}

fn rebind_kind(kind: EvidenceKind, file: FileId) -> EvidenceKind {
    match kind {
        EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
            lower_span,
            upper_span,
            activation,
        }) => EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
            lower_span: span_file(lower_span, file),
            upper_span: span_file(upper_span, file),
            activation,
        }),
        EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
            target_span,
            name_hash,
        }) => EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
            target_span: span_file(target_span, file),
            name_hash,
        }),
        EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
            target_span,
            receiver_type_hash,
            method_hash,
        }) => EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
            target_span: span_file(target_span, file),
            receiver_type_hash,
            method_hash,
        }),
        EvidenceKind::PromiseSettledValue(PromiseSettledValueEvidenceKind {
            channel,
            payload_span,
            payload_kind,
        }) => EvidenceKind::PromiseSettledValue(PromiseSettledValueEvidenceKind {
            channel,
            payload_span: span_file(payload_span, file),
            payload_kind,
        }),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        BoundOrderGuardActivation, Corpus, EvidenceId, EvidenceProvenance, EvidenceStatus, Lang,
    };

    fn lower(file: FileId, path: &str, interner: &Interner) -> Il {
        nose_frontend::lower_source(
            file,
            path,
            b"def total(xs):\n    return sum(xs)\n",
            Lang::Python,
            interner,
        )
        .unwrap()
    }

    fn add_span_evidence(il: &mut Il) {
        let span = il.node(il.root).span;
        il.evidence.push(EvidenceRecord::new(
            EvidenceId(il.evidence.len() as u32),
            EvidenceAnchor::source_span(span),
            EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
                lower_span: span,
                upper_span: span,
                activation: BoundOrderGuardActivation::WhenTrue,
            }),
            EvidenceProvenance::builtin("nose.test", "portable"),
            Vec::new(),
            EvidenceStatus::Asserted,
        ));
    }

    #[test]
    fn digest_ignores_path_file_and_interner_order_but_covers_evidence() {
        let first_interner = Interner::new();
        first_interner.intern("inserted-before-source");
        let mut first = lower(FileId(7), "/checkout-a/src/a.py", &first_interner);
        add_span_evidence(&mut first);

        let second_interner = Interner::new();
        second_interner.intern("different-prior-symbol");
        second_interner.intern("another-prior-symbol");
        let mut second = lower(FileId(91), "/checkout-b/src/a.py", &second_interner);
        add_span_evidence(&mut second);
        assert_eq!(
            semantic_digest(&first, &first_interner),
            semantic_digest(&second, &second_interner)
        );

        second.evidence[0].status = EvidenceStatus::Ambiguous;
        assert_ne!(
            semantic_digest(&first, &first_interner),
            semantic_digest(&second, &second_interner),
            "an evidence-only change must invalidate resolved content"
        );
    }

    #[test]
    fn source_digest_covers_language_and_exact_bytes() {
        let source = b"def f():\n    return 1\n";
        assert_eq!(
            source_digest(Lang::Python, source),
            source_digest(Lang::Python, source)
        );
        assert_ne!(
            source_digest(Lang::Python, source),
            source_digest(Lang::Python, b"def f():\n    return 2\n")
        );
        assert_ne!(
            source_digest(Lang::Python, source),
            source_digest(Lang::Ruby, source)
        );
    }

    #[test]
    fn round_trip_rebinds_every_path_file_and_symbol() {
        let source_interner = Interner::new();
        let mut original = lower(FileId(23), "/old/root/a.py", &source_interner);
        add_span_evidence(&mut original);
        let encoded = encode(&original, &source_interner).unwrap();

        let target_interner = Interner::new();
        target_interner.intern("different-target-order");
        let restored = decode(
            &encoded,
            &target_interner,
            FileId(4),
            "/new/root/a.py".to_owned(),
        )
        .unwrap();
        assert_eq!(restored.file, FileId(4));
        assert_eq!(restored.meta.path, "/new/root/a.py");
        assert!(restored
            .nodes
            .iter()
            .all(|node| node.span.file == FileId(4)));
        assert!(restored.evidence.iter().all(|record| {
            record.anchor.span().file == FileId(4)
                && match record.kind {
                    EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
                        lower_span,
                        upper_span,
                        ..
                    }) => lower_span.file == FileId(4) && upper_span.file == FileId(4),
                    _ => true,
                }
        }));
        assert_eq!(
            semantic_digest(&original, &source_interner),
            semantic_digest(&restored, &target_interner)
        );
    }

    fn report_json(corpus: &Corpus) -> Vec<u8> {
        let opts = nose_detect::DetectOptions::default();
        let features = nose_detect::corpus_features(corpus, &opts);
        let detector =
            nose_detect::StructuralDetector::candidates(0.75).with_threshold(opts.threshold);
        let report = nose_detect::detect_from_units(
            features.units,
            features.files,
            &features.streams,
            &opts,
            &detector,
        )
        .0;
        serde_json::to_vec(&report).unwrap()
    }

    #[test]
    fn raw_and_resolved_round_trips_preserve_detector_json() {
        let root = std::env::temp_dir().join(format!("nose_portable_il_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.py"), "def total(xs):\n    return sum(xs)\n").unwrap();
        std::fs::write(
            root.join("b.py"),
            "def aggregate(xs):\n    return sum(xs)\n",
        )
        .unwrap();

        let raw = nose_frontend::lower_corpus_raw_filtered(&[root.as_path()], &[]);
        assert_round_trip_report(&raw);
        let mut resolved = raw.clone();
        nose_frontend::resolve_corpus(&mut resolved);
        assert_round_trip_report(&resolved);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn assert_round_trip_report(corpus: &Corpus) {
        let target_interner = Interner::new();
        let files = corpus
            .files
            .iter()
            .map(|il| {
                decode(
                    &encode(il, &corpus.interner).unwrap(),
                    &target_interner,
                    il.file,
                    il.meta.path.clone(),
                )
                .unwrap()
            })
            .collect();
        let restored = Corpus::new(target_interner, files);
        assert_eq!(report_json(corpus), report_json(&restored));
    }

    #[test]
    fn region_identity_is_stable_across_checkout_and_content_edits() {
        let interner = Interner::new();
        let first = lower(FileId(0), "/a/component.py", &interner);
        let second = nose_frontend::lower_source(
            FileId(9),
            "/b/component.py",
            b"def renamed(values):\n    return max(values)\n",
            Lang::Python,
            &interner,
        )
        .unwrap();
        assert_eq!(region_identity(&first), region_identity(&second));
    }

    #[test]
    fn embedded_regions_have_stable_distinct_subidentities() {
        let source_a = br#"<script>function a() { return 1; }</script>
<style>.a { color: red; }</style><main>Hello</main>"#;
        let source_b = br#"<main>Hello again</main>
<style>.b { color: blue; }</style><script>function b() { return 2; }</script>"#;
        let first_interner = Interner::new();
        let second_interner = Interner::new();
        let first = nose_frontend::lower_source_regions(
            FileId(1),
            "/checkout-a/page.html",
            source_a,
            Lang::Html,
            &first_interner,
        );
        let second = nose_frontend::lower_source_regions(
            FileId(99),
            "/checkout-b/page.html",
            source_b,
            Lang::Html,
            &second_interner,
        );
        let identities = |regions: &[Il]| {
            let mut values = regions
                .iter()
                .map(|il| (il.meta.lang.name(), region_identity(il).hex()))
                .collect::<Vec<_>>();
            values.sort();
            values
        };
        let first_ids = identities(&first);
        assert_eq!(first_ids, identities(&second));
        assert_eq!(
            first_ids
                .iter()
                .map(|(_, digest)| digest)
                .collect::<BTreeSet<_>>()
                .len(),
            first_ids.len(),
            "each embedded language region needs its own stable subidentity"
        );
    }
}
