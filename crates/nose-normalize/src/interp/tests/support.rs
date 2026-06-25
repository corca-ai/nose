use super::*;

pub(super) fn append_test_python_iterator_builtin_source_dependencies(
    il: &mut Il,
    call: NodeId,
    builtin: Builtin,
    next_id: &mut u32,
    dependencies: &mut Vec<EvidenceId>,
) {
    if il.meta.lang != Lang::Python {
        return;
    }
    let source_args: &[usize] = match builtin {
        Builtin::Zip => &[0, 1],
        Builtin::Enumerate | Builtin::Any | Builtin::All => &[0],
        _ => return,
    };
    let children = il.children(call).to_vec();
    for &arg_idx in source_args {
        let Some(&source) = children.get(arg_idx) else {
            continue;
        };
        let id = EvidenceId(*next_id);
        il.evidence.push(test_domain_record(
            *next_id,
            il,
            source,
            DomainEvidence::Collection,
        ));
        *next_id += 1;
        dependencies.push(id);
    }
}
