//! Deterministic, pair-local evidence for intentional clone variants.
//!
//! This module does not decide the active divergent-edit tier. It records strong
//! disqualifiers, weak review hints, and uncertainty separately so #852 can price a
//! frozen policy without turning names, paths, or repository-specific prose into gate
//! authority.

mod source;

use self::source::source_signals;
use crate::source_lines::FileLineCache;
use nose_detect::{GradedWitness, Loc};
use nose_il::{UnitEvidenceFlag, UnitOrigin};
use std::collections::BTreeSet;
use std::path::Path;

const DETAIL_CAP: usize = 8;
const DETAIL_CHARS: usize = 160;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum VariantEvidenceStatus {
    None,
    Advisory,
    Disqualifying,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum VariantEvidenceStrength {
    Weak,
    Strong,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum VariantSignalCode {
    ReferentMismatch,
    DecoratorMismatch,
    AsyncRoleMismatch,
    EffectRoleMismatch,
    ProtocolRoleMismatch,
    DisjointPlatformGuard,
    NameMismatch,
    PathMismatch,
    VersionLabelMismatch,
}

impl VariantSignalCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReferentMismatch => "referent-mismatch",
            Self::DecoratorMismatch => "decorator-mismatch",
            Self::AsyncRoleMismatch => "async-role-mismatch",
            Self::EffectRoleMismatch => "effect-role-mismatch",
            Self::ProtocolRoleMismatch => "protocol-role-mismatch",
            Self::DisjointPlatformGuard => "disjoint-platform-guard",
            Self::NameMismatch => "name-mismatch",
            Self::PathMismatch => "path-mismatch",
            Self::VersionLabelMismatch => "version-label-mismatch",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum VariantCaveatCode {
    SourceUnavailable,
    ProjectionUnavailable,
    AlignmentUnavailable,
    LossyProjection,
    UnresolvedReferent,
    Truncated,
    ConflictingPlatformGuard,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
pub(crate) struct VariantSignal {
    pub(crate) code: VariantSignalCode,
    pub(crate) strength: VariantEvidenceStrength,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) changed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) skipped: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
pub(crate) struct VariantCaveat {
    pub(crate) code: VariantCaveatCode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) details: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct VariantEvidence {
    pub(crate) status: VariantEvidenceStatus,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) signals: Vec<VariantSignal>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) caveats: Vec<VariantCaveat>,
}

pub(super) struct VariantSourceContext<'a> {
    pub(super) current_root: &'a Path,
    pub(super) base_root: &'a Path,
    pub(super) lines: &'a mut FileLineCache,
}

impl VariantEvidence {
    fn empty() -> Self {
        Self {
            status: VariantEvidenceStatus::None,
            signals: Vec::new(),
            caveats: Vec::new(),
        }
    }

    fn signal(
        &mut self,
        code: VariantSignalCode,
        strength: VariantEvidenceStrength,
        changed: impl IntoIterator<Item = String>,
        skipped: impl IntoIterator<Item = String>,
    ) {
        self.signals.push(VariantSignal {
            code,
            strength,
            changed: bounded(changed),
            skipped: bounded(skipped),
        });
        self.finish();
    }

    pub(super) fn caveat(
        &mut self,
        code: VariantCaveatCode,
        details: impl IntoIterator<Item = String>,
    ) {
        self.caveats.push(VariantCaveat {
            code,
            details: bounded(details),
        });
        self.finish();
    }

    fn finish(&mut self) {
        self.signals.sort();
        self.signals.dedup();
        self.caveats.sort();
        self.caveats.dedup();
        self.status = if self
            .signals
            .iter()
            .any(|signal| signal.strength == VariantEvidenceStrength::Strong)
        {
            VariantEvidenceStatus::Disqualifying
        } else if self.signals.is_empty() && self.caveats.is_empty() {
            VariantEvidenceStatus::None
        } else {
            VariantEvidenceStatus::Advisory
        };
    }

    pub(crate) fn concise_label(&self) -> Option<String> {
        if self.status == VariantEvidenceStatus::None {
            return None;
        }
        let prefix = match self.status {
            VariantEvidenceStatus::Disqualifying => "strong variant",
            VariantEvidenceStatus::Advisory => "variant advisory",
            VariantEvidenceStatus::None => return None,
        };
        let codes = self
            .signals
            .iter()
            .map(|signal| signal.code.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if codes.is_empty() {
            Some(format!("{prefix} (evidence unavailable)"))
        } else {
            Some(format!("{prefix} ({codes})"))
        }
    }
}

pub(super) fn initial_evidence(changed: &Loc, skipped: &Loc) -> VariantEvidence {
    let mut evidence = VariantEvidence::empty();
    weak_identity_signals(&mut evidence, changed, skipped);
    origin_signals(&mut evidence, changed.origin, skipped.origin);
    evidence
}

pub(super) fn enrich_source(
    evidence: &mut VariantEvidence,
    changed: &super::Site,
    current_path: &str,
    current_span: Option<(u32, u32)>,
    skipped: &super::Site,
    context: &mut VariantSourceContext<'_>,
) {
    source_signals(
        evidence,
        changed,
        current_path,
        current_span,
        skipped,
        context,
    );
}

pub(super) fn enrich_projected(
    evidence: &mut VariantEvidence,
    witness: Option<&GradedWitness>,
    changed_origin: UnitOrigin,
    skipped_origin: UnitOrigin,
    projection_details: &[String],
    truncated: bool,
) {
    origin_signals(evidence, changed_origin, skipped_origin);
    let Some(witness) = witness else {
        let code = if projection_details.is_empty() {
            VariantCaveatCode::AlignmentUnavailable
        } else {
            VariantCaveatCode::ProjectionUnavailable
        };
        evidence.caveat(code, projection_details.iter().cloned());
        if truncated {
            evidence.caveat(VariantCaveatCode::Truncated, std::iter::empty());
        }
        return;
    };
    if !witness.referent_mismatches.is_empty() {
        let names = witness.referent_mismatches.iter().cloned();
        evidence.signal(
            VariantSignalCode::ReferentMismatch,
            VariantEvidenceStrength::Strong,
            names.clone(),
            names,
        );
    }
    if witness.patterns.contains(&"async-mirror") {
        evidence.signal(
            VariantSignalCode::AsyncRoleMismatch,
            VariantEvidenceStrength::Strong,
            ["one-sided-await-observed".to_string()],
            ["one-sided-await-observed".to_string()],
        );
    }
    if witness.patterns.contains(&"effects-reordered") {
        evidence.signal(
            VariantSignalCode::EffectRoleMismatch,
            VariantEvidenceStrength::Strong,
            ["effect-order-a".to_string()],
            ["effect-order-b".to_string()],
        );
    }
    if !witness.caveat_names.is_empty() {
        evidence.caveat(
            VariantCaveatCode::UnresolvedReferent,
            witness.caveat_names.iter().cloned(),
        );
    }
    if witness.modeled_caveat {
        evidence.caveat(VariantCaveatCode::LossyProjection, std::iter::empty());
    }
    if truncated {
        evidence.caveat(VariantCaveatCode::Truncated, std::iter::empty());
    }
}

fn origin_signals(evidence: &mut VariantEvidence, changed: UnitOrigin, skipped: UnitOrigin) {
    // A missing/legacy origin is not proof of the negative role. The projection
    // caveat remains advisory instead of treating unknown as sync/non-throws.
    if changed.is_unknown() || skipped.is_unknown() {
        return;
    }
    let async_roles = roles(changed, skipped, UnitEvidenceFlag::Async, "async", "sync");
    if let Some((changed_role, skipped_role)) = async_roles {
        evidence.signal(
            VariantSignalCode::AsyncRoleMismatch,
            VariantEvidenceStrength::Strong,
            [changed_role],
            [skipped_role],
        );
    }
    let throw_roles = roles(
        changed,
        skipped,
        UnitEvidenceFlag::Throws,
        "throws",
        "non-throws",
    );
    if let Some((changed_role, skipped_role)) = throw_roles {
        evidence.signal(
            VariantSignalCode::EffectRoleMismatch,
            VariantEvidenceStrength::Strong,
            [changed_role],
            [skipped_role],
        );
    }
    if let (Some(changed_role), Some(skipped_role)) =
        (protocol_role(changed), protocol_role(skipped))
    {
        if changed_role != skipped_role {
            evidence.signal(
                VariantSignalCode::ProtocolRoleMismatch,
                VariantEvidenceStrength::Strong,
                [changed_role.to_string()],
                [skipped_role.to_string()],
            );
        }
    }
}

fn roles(
    changed: UnitOrigin,
    skipped: UnitOrigin,
    flag: UnitEvidenceFlag,
    positive: &str,
    negative: &str,
) -> Option<(String, String)> {
    match (changed.has_evidence(flag), skipped.has_evidence(flag)) {
        (true, false) => Some((positive.to_string(), negative.to_string())),
        (false, true) => Some((negative.to_string(), positive.to_string())),
        _ => None,
    }
}

fn protocol_role(origin: UnitOrigin) -> Option<&'static str> {
    [
        (
            UnitEvidenceFlag::ProtocolRequirement,
            "protocol-requirement",
        ),
        (UnitEvidenceFlag::ProtocolExtension, "protocol-extension"),
        (
            UnitEvidenceFlag::ConcreteTypeExtension,
            "concrete-type-extension",
        ),
        (
            UnitEvidenceFlag::InterfaceDefaultMethod,
            "interface-default-method",
        ),
        (
            UnitEvidenceFlag::InterfaceStaticMethod,
            "interface-static-method",
        ),
        (
            UnitEvidenceFlag::InterfacePrivateMethod,
            "interface-private-method",
        ),
    ]
    .into_iter()
    .find_map(|(flag, role)| origin.has_evidence(flag).then_some(role))
}

fn weak_identity_signals(evidence: &mut VariantEvidence, changed: &Loc, skipped: &Loc) {
    let changed_name = effective_name(changed);
    let skipped_name = effective_name(skipped);
    if let (Some(changed_name), Some(skipped_name)) = (changed_name, skipped_name) {
        if changed_name != skipped_name {
            evidence.signal(
                VariantSignalCode::NameMismatch,
                VariantEvidenceStrength::Weak,
                [changed_name.to_string()],
                [skipped_name.to_string()],
            );
        }
    }
    if changed.file != skipped.file {
        evidence.signal(
            VariantSignalCode::PathMismatch,
            VariantEvidenceStrength::Weak,
            [changed.file.clone()],
            [skipped.file.clone()],
        );
    }
    let changed_versions = version_labels(changed);
    let skipped_versions = version_labels(skipped);
    if changed_versions != skipped_versions
        && (!changed_versions.is_empty() || !skipped_versions.is_empty())
    {
        evidence.signal(
            VariantSignalCode::VersionLabelMismatch,
            VariantEvidenceStrength::Weak,
            changed_versions,
            skipped_versions,
        );
    }
}

fn effective_name(loc: &Loc) -> Option<&str> {
    loc.name
        .as_deref()
        .or_else(|| loc.enclosing_unit.as_ref()?.name.as_deref())
}

fn version_labels(loc: &Loc) -> Vec<String> {
    let mut labels = BTreeSet::new();
    let text = format!("{} {}", loc.file, effective_name(loc).unwrap_or_default());
    for token in text.split(|character: char| !character.is_ascii_alphanumeric()) {
        let lower = token.to_ascii_lowercase();
        let numeric_version = lower
            .strip_prefix('v')
            .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()));
        if numeric_version || matches!(lower.as_str(), "legacy" | "modern") {
            labels.insert(lower);
        }
    }
    labels.into_iter().collect()
}

fn bounded(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| cap_detail(&value))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values.truncate(DETAIL_CAP);
    values
}

fn cap_detail(value: &str) -> String {
    if value.chars().count() <= DETAIL_CHARS {
        return value.to_string();
    }
    let end = value
        .char_indices()
        .nth(DETAIL_CHARS)
        .map_or(value.len(), |(index, _)| index);
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(file: &str, name: &str) -> Loc {
        Loc {
            file: file.to_string(),
            start_line: 1,
            end_line: 2,
            lang: "go".to_string(),
            kind: nose_il::UnitKind::Function,
            origin: UnitOrigin::unknown(),
            name: Some(name.to_string()),
            sem: 1,
            span_lines: 2,
            span_tokens: 2,
            is_fragment: false,
            fragment_kind: None,
            reason_code: None,
            enclosing_unit: None,
            in_test_module: false,
            looks_generated: false,
            shared_subdag: None,
        }
    }

    #[test]
    fn version_labels_are_exact_weak_hints() {
        let changed = loc("completion_v2.go", "renderV2");
        let skipped = loc("completion.go", "render");
        let mut evidence = VariantEvidence::empty();
        weak_identity_signals(&mut evidence, &changed, &skipped);
        assert_eq!(evidence.status, VariantEvidenceStatus::Advisory);
        assert!(evidence.signals.iter().any(|signal| {
            signal.code == VariantSignalCode::VersionLabelMismatch
                && signal.strength == VariantEvidenceStrength::Weak
        }));
    }

    #[test]
    fn weak_name_only_is_never_disqualifying() {
        let changed = loc("same.go", "changes");
        let skipped = loc("same.go", "resets");
        let mut evidence = VariantEvidence::empty();
        weak_identity_signals(&mut evidence, &changed, &skipped);
        assert_eq!(evidence.status, VariantEvidenceStatus::Advisory);
        assert_eq!(evidence.signals.len(), 1);
        assert_eq!(evidence.signals[0].code, VariantSignalCode::NameMismatch);
        assert_eq!(evidence.signals[0].strength, VariantEvidenceStrength::Weak);
    }

    #[test]
    fn referent_async_and_protocol_evidence_name_strong_roles() {
        let witness = GradedWitness {
            holes: 0,
            spots: Vec::new(),
            patterns: vec!["referent-mismatch"],
            referent_mismatches: vec!["handler".to_string()],
            caveat_names: Vec::new(),
            equal_modulo_holes: false,
            modeled_caveat: false,
        };
        let changed = UnitOrigin::unknown()
            .with_evidence(UnitEvidenceFlag::Async)
            .with_evidence(UnitEvidenceFlag::ProtocolRequirement);
        let skipped = UnitOrigin::unknown().with_evidence(UnitEvidenceFlag::ProtocolExtension);
        let mut evidence = VariantEvidence::empty();
        enrich_projected(&mut evidence, Some(&witness), changed, skipped, &[], false);
        assert_eq!(evidence.status, VariantEvidenceStatus::Disqualifying);
        for code in [
            VariantSignalCode::ReferentMismatch,
            VariantSignalCode::AsyncRoleMismatch,
            VariantSignalCode::ProtocolRoleMismatch,
        ] {
            assert!(evidence.signals.iter().any(|signal| signal.code == code));
        }
    }

    #[test]
    fn variant_contract_codes_are_closed_and_kebab_cased() {
        let signals = [
            VariantSignalCode::ReferentMismatch,
            VariantSignalCode::DecoratorMismatch,
            VariantSignalCode::AsyncRoleMismatch,
            VariantSignalCode::EffectRoleMismatch,
            VariantSignalCode::ProtocolRoleMismatch,
            VariantSignalCode::DisjointPlatformGuard,
            VariantSignalCode::NameMismatch,
            VariantSignalCode::PathMismatch,
            VariantSignalCode::VersionLabelMismatch,
        ];
        assert_eq!(
            signals.map(VariantSignalCode::as_str),
            [
                "referent-mismatch",
                "decorator-mismatch",
                "async-role-mismatch",
                "effect-role-mismatch",
                "protocol-role-mismatch",
                "disjoint-platform-guard",
                "name-mismatch",
                "path-mismatch",
                "version-label-mismatch",
            ]
        );
        let caveats = serde_json::to_value([
            VariantCaveatCode::SourceUnavailable,
            VariantCaveatCode::ProjectionUnavailable,
            VariantCaveatCode::AlignmentUnavailable,
            VariantCaveatCode::LossyProjection,
            VariantCaveatCode::UnresolvedReferent,
            VariantCaveatCode::Truncated,
            VariantCaveatCode::ConflictingPlatformGuard,
        ])
        .unwrap();
        assert_eq!(
            caveats,
            serde_json::json!([
                "source-unavailable",
                "projection-unavailable",
                "alignment-unavailable",
                "lossy-projection",
                "unresolved-referent",
                "truncated",
                "conflicting-platform-guard"
            ])
        );
    }
}
