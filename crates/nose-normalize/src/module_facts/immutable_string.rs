use super::*;
use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceAnchor, EvidenceKind, EvidenceStatus, Lang,
    SourceBindingKind, Span, TypeEvidenceKind,
};

/// A direct immutable module string binding proven from the same file-level assignment and
/// mutation boundary used by value-fingerprint seeding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableModuleStringBinding {
    pub name: Symbol,
    pub statement: NodeId,
    pub literal_hash: u64,
}

/// Enumerate direct module bindings whose runtime string value is exact and immutable.
/// This intentionally supports only a direct string literal. Aliases, computed initializers,
/// duplicate definitions, callable-name collisions, and any in-file mutation/escape are omitted.
/// The interpreter consumes this proof instead of treating an unresolved global name as a
/// guessed value; the value graph already uses the same module scope/mutation boundary.
pub fn immutable_module_string_bindings(
    il: &Il,
    interner: &Interner,
) -> Vec<ImmutableModuleStringBinding> {
    if il.meta.lang != Lang::Swift {
        return Vec::new();
    }
    let local_scope = local_scope_nodes(il);
    let top_level = top_level_statements_for(il);
    let is_top_level = top_level_node_bitmap(il, &top_level);
    let counts = assignment_name_counts(&top_level, |statement| {
        assignment_name_in_scope(il, statement, &local_scope)
    });
    let candidates = unique_non_unit_assignment_names(il, &counts);
    let direct_definitions =
        direct_assignment_definitions_in_scope(il, &is_top_level, &local_scope);
    let mutated = collect_module_mutations_in_scope_with_direct_definitions(
        il,
        interner,
        &candidates,
        &is_top_level,
        &local_scope,
        &direct_definitions,
    );

    let mut bindings = Vec::new();
    for &statement in &top_level {
        let Some(name) = assignment_name_in_scope(il, statement, &local_scope) else {
            continue;
        };
        if !candidates.contains(&name)
            || mutated.contains(&name)
            || nose_semantics::source_binding_at_node(il, statement)
                != Some(SourceBindingKind::ImmutableDeclaration)
        {
            continue;
        }
        let Some(&rhs) = il.children(statement).get(1) else {
            continue;
        };
        let Payload::LitStr(literal_hash) = il.node(rhs).payload else {
            continue;
        };
        let Some((lhs, _)) = il.assignment_var_parts(statement) else {
            continue;
        };
        if !swift_binding_has_exact_string_type(il, interner, name, il.node(lhs).span) {
            continue;
        }
        bindings.push(ImmutableModuleStringBinding {
            name,
            statement,
            literal_hash,
        });
    }
    bindings.sort_unstable_by_key(|binding| interner.symbol_hash(binding.name));
    bindings
}

fn swift_binding_has_exact_string_type(
    il: &Il,
    interner: &Interner,
    name: Symbol,
    binding_span: Span,
) -> bool {
    let local_hash = stable_symbol_hash(interner.resolve(name));
    let records: Vec<_> = il
        .evidence
        .iter()
        .filter(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Binding {
                    local_hash: candidate,
                    span,
                } if candidate == local_hash && span == binding_span
            )
        })
        .collect();
    if records.iter().any(|record| {
        matches!(record.kind, EvidenceKind::Domain(domain) if domain != DomainEvidence::String)
    }) {
        return false;
    }
    let explicit = records.iter().any(|record| {
        record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftExplicitBindingType)
    });
    if !explicit {
        // An uncontextualized Swift string literal defaults to `Swift.String`.
        return true;
    }
    records.iter().any(|record| {
        matches!(
            record.kind,
            EvidenceKind::Type(
                TypeEvidenceKind::SwiftUnqualifiedStringBinding
                    | TypeEvidenceKind::SwiftQualifiedStringBinding
            )
        ) && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    }) && records.iter().any(|record| {
        record.kind == EvidenceKind::Domain(DomainEvidence::String)
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record)
    })
}
