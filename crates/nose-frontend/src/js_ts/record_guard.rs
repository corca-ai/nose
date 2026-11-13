use super::globals::{enclosing_function_prefix_has_binding_ident, file_prefix_has_binding_ident};
use super::syntax::{compact_js_expr, simple_js_ident, strip_outer_parens_owned};
use crate::lower::Lowering;
use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceKind, GuardEvidenceKind, JsRecordGuardComparison,
    JsRecordGuardNullCheck, NodeId, NodeKind, Payload, SymbolEvidenceKind,
};
use nose_semantics::{js_array_is_array_contract, js_boolean_coercion_contract};
use tree_sitter::Node as TsNode;

pub(super) fn lower_record_shape_guard(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let text = compact_js_expr(lo.text(node));
    let clauses: Vec<String> = text
        .split("&&")
        .map(strip_outer_parens_owned)
        .filter(|s| !s.is_empty())
        .collect();
    if clauses.len() != 3 {
        return None;
    }

    let mut ident: Option<String> = None;
    let mut has_typeof_object = false;
    let mut null_check = None;
    let mut has_not_array = false;
    let mut comparison = JsRecordGuardComparison::StrictOnly;
    for clause in clauses {
        let parsed = record_guard_clause(&clause)?;
        let name = parsed.name;
        if !simple_js_ident(&name) {
            return None;
        }
        match &ident {
            Some(current) if current != &name => return None,
            None => ident = Some(name.clone()),
            _ => {}
        }
        comparison = merge_record_guard_comparison(comparison, parsed.comparison);
        match parsed.kind {
            RecordGuardClause::TypeofObject => has_typeof_object = true,
            RecordGuardClause::NonNullOrTruthy { kind } => null_check = Some(kind),
            RecordGuardClause::NotArray => has_not_array = true,
        }
    }

    if !(has_typeof_object && null_check.is_some() && has_not_array) {
        return None;
    }
    let array_contract = js_array_is_array_contract(lo.lang, "Array", "isArray", 1)?;
    if array_contract.requires_unshadowed_receiver
        && (file_prefix_has_binding_ident(lo, node, array_contract.receiver)
            || enclosing_function_prefix_has_binding_ident(lo, node, array_contract.receiver))
    {
        return None;
    }
    let null_check = null_check?;
    let requires_boolean_global = null_check == JsRecordGuardNullCheck::BooleanGlobalTruthy;
    let boolean_contract = requires_boolean_global
        .then(|| js_boolean_coercion_contract(lo.lang, "Boolean", 1))
        .flatten();
    if requires_boolean_global && boolean_contract.is_none() {
        return None;
    }
    if boolean_contract.is_some_and(|contract| {
        contract.requires_unshadowed_function
            && (file_prefix_has_binding_ident(lo, node, contract.function)
                || enclosing_function_prefix_has_binding_ident(lo, node, contract.function))
    }) {
        return None;
    }
    let ident = ident?;
    let span = lo.span(node);
    let value = lo.var(&ident, span);
    let object = lo.str_content("object", span);
    let non_null = lo.str_content("non_null", span);
    let not_array = lo.str_content("not_array", span);
    let tag = lo.sym("record_guard");
    let guard = lo.add(
        NodeKind::Seq,
        Payload::Name(tag),
        span,
        &[value, object, non_null, not_array],
    );
    let array_dependency = lo.record_qualified_global_source_symbol(
        span,
        array_contract.qualified_path,
        "record_guard_array_is_array_api",
    );
    let mut dependencies = vec![array_dependency];
    if let Some(contract) = boolean_contract {
        dependencies.push(lo.record_evidence(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash(contract.function),
            }),
            "record_guard_boolean_api",
        ));
    }
    lo.record_evidence_with_dependencies(
        EvidenceAnchor::sequence(span),
        EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape {
            subject_hash: stable_symbol_hash(&ident),
            null_check,
            comparison,
        }),
        "record_guard_js_shape",
        dependencies,
    );
    Some(guard)
}

#[derive(Clone, Copy)]
enum RecordGuardClause {
    TypeofObject,
    NonNullOrTruthy { kind: JsRecordGuardNullCheck },
    NotArray,
}

struct ParsedRecordGuardClause {
    kind: RecordGuardClause,
    name: String,
    comparison: JsRecordGuardComparison,
}

fn record_guard_clause(clause: &str) -> Option<ParsedRecordGuardClause> {
    parse_typeof_object_clause(clause)
        .map(|(name, comparison)| ParsedRecordGuardClause {
            kind: RecordGuardClause::TypeofObject,
            name,
            comparison,
        })
        .or_else(|| {
            parse_non_null_clause(clause).map(|(name, kind, comparison)| ParsedRecordGuardClause {
                kind: RecordGuardClause::NonNullOrTruthy { kind },
                name,
                comparison,
            })
        })
        .or_else(|| {
            parse_truthy_clause(clause).map(|(name, kind)| ParsedRecordGuardClause {
                kind: RecordGuardClause::NonNullOrTruthy { kind },
                name,
                comparison: JsRecordGuardComparison::StrictOnly,
            })
        })
        .or_else(|| {
            parse_not_array_clause(clause).map(|(name, comparison)| ParsedRecordGuardClause {
                kind: RecordGuardClause::NotArray,
                name,
                comparison,
            })
        })
}

fn parse_typeof_object_clause(clause: &str) -> Option<(String, JsRecordGuardComparison)> {
    for op in ["===", "=="] {
        let comparison = record_guard_comparison_for_op(op);
        if let Some(rest) = clause.strip_prefix("typeof ") {
            let (name, value) = rest.split_once(op)?;
            if is_object_literal(value) {
                return Some((name.to_string(), comparison));
            }
        }
        for object_lit in ["'object'", "\"object\""] {
            let prefix = format!("{object_lit}{op}typeof ");
            if let Some(name) = clause.strip_prefix(&prefix) {
                return Some((name.to_string(), comparison));
            }
        }
    }
    None
}

fn parse_non_null_clause(
    clause: &str,
) -> Option<(String, JsRecordGuardNullCheck, JsRecordGuardComparison)> {
    for op in ["!==", "!="] {
        let null_check = match op {
            "!==" => JsRecordGuardNullCheck::StrictNonNull,
            "!=" => JsRecordGuardNullCheck::LooseNonNull,
            _ => unreachable!(),
        };
        let comparison = record_guard_comparison_for_op(op);
        if let Some((name, "null")) = clause.split_once(op) {
            return Some((name.to_string(), null_check, comparison));
        }
        let prefix = format!("null{op}");
        if let Some(name) = clause.strip_prefix(&prefix) {
            return Some((name.to_string(), null_check, comparison));
        }
    }
    None
}

fn parse_truthy_clause(clause: &str) -> Option<(String, JsRecordGuardNullCheck)> {
    if let Some(name) = clause.strip_prefix("!!") {
        return Some((
            name.to_string(),
            JsRecordGuardNullCheck::DoubleNegationTruthy,
        ));
    }
    clause
        .strip_prefix("Boolean(")
        .and_then(|inner| inner.strip_suffix(')'))
        .map(|name| {
            (
                name.to_string(),
                JsRecordGuardNullCheck::BooleanGlobalTruthy,
            )
        })
}

fn parse_not_array_clause(clause: &str) -> Option<(String, JsRecordGuardComparison)> {
    if let Some(name) = clause
        .strip_prefix("!Array.isArray(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        return Some((name.to_string(), JsRecordGuardComparison::StrictOnly));
    }
    for op in ["===", "=="] {
        let comparison = record_guard_comparison_for_op(op);
        if let Some(call) = clause.strip_suffix(&format!("{op}false")) {
            if let Some(name) = call
                .strip_prefix("Array.isArray(")
                .and_then(|inner| inner.strip_suffix(')'))
            {
                return Some((name.to_string(), comparison));
            }
        }
        let prefix = format!("false{op}Array.isArray(");
        if let Some(name) = clause
            .strip_prefix(&prefix)
            .and_then(|inner| inner.strip_suffix(')'))
        {
            return Some((name.to_string(), comparison));
        }
    }
    None
}

fn record_guard_comparison_for_op(op: &str) -> JsRecordGuardComparison {
    match op {
        "==" | "!=" => JsRecordGuardComparison::LooseEqualityAllowed,
        _ => JsRecordGuardComparison::StrictOnly,
    }
}

fn merge_record_guard_comparison(
    left: JsRecordGuardComparison,
    right: JsRecordGuardComparison,
) -> JsRecordGuardComparison {
    if matches!(
        (left, right),
        (JsRecordGuardComparison::LooseEqualityAllowed, _)
            | (_, JsRecordGuardComparison::LooseEqualityAllowed)
    ) {
        JsRecordGuardComparison::LooseEqualityAllowed
    } else {
        JsRecordGuardComparison::StrictOnly
    }
}

fn is_object_literal(value: &str) -> bool {
    matches!(value, "'object'" | "\"object\"")
}
