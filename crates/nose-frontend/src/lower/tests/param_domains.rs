use super::*;

#[test]
fn parameter_type_domains_are_dependency_backed_and_not_substring_guesses() {
    let interner = Interner::new();
    assert_python_typing_alias_param_domains(&interner);
    assert_python_stdlib_pack_param_domains(&interner);
    assert_ts_and_java_param_domains(&interner);
    assert_rust_result_param_domains(&interner);
}

fn import_backed_param_domain_pack_hash(
    evidence: &[EvidenceRecord],
    exported: &str,
    domain: DomainEvidence,
) -> Option<u64> {
    import_backed_param_domain_provenance(evidence, exported, domain)
        .map(|(pack_hash, _)| pack_hash)
}

fn import_backed_param_domain_provenance(
    evidence: &[EvidenceRecord],
    exported: &str,
    domain: DomainEvidence,
) -> Option<(u64, u64)> {
    let import_ids = imported_binding_symbol_ids(evidence, "typing", exported);
    assert_eq!(import_ids.len(), 1);
    let py_domains = param_domain_records(evidence, domain);
    assert_eq!(py_domains.len(), 1);
    assert_eq!(py_domains[0].dependencies, import_ids);
    py_domains[0]
        .provenance
        .pack_hash
        .zip(py_domains[0].provenance.rule_hash)
}

fn assert_python_typing_alias_param_domains(interner: &Interner) {
    let py_alias = lower_fixture(
        "typing_alias.py",
        b"from typing import List as L\ndef f(xs: L[int]):\n    return len(xs)\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        import_backed_param_domain_provenance(
            &py_alias.evidence,
            "List",
            DomainEvidence::Collection
        ),
        Some((
            stable_symbol_hash(nose_semantics::PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID),
            stable_symbol_hash(nose_semantics::PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID)
        )),
        "imported Python stdlib type aliases should carry pack and producer provenance"
    );

    let py_direct_import_alias = lower_fixture(
        "typing_direct_import_alias.py",
        b"from typing import List\ndef f(xs: List[int]):\n    return len(xs)\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        import_backed_param_domain_pack_hash(
            &py_direct_import_alias.evidence,
            "List",
            DomainEvidence::Collection
        ),
        Some(stable_symbol_hash(
            nose_semantics::PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID
        )),
        "a direct imported alias should not fall through to first-party text heuristics"
    );

    let py_iter_alias = lower_fixture(
        "typing_iter_alias.py",
        b"from typing import Iterable as I\ndef f(xs: I[int]):\n    return xs\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        import_backed_param_domain_pack_hash(
            &py_iter_alias.evidence,
            "Iterable",
            DomainEvidence::Iterable
        ),
        Some(stable_symbol_hash(
            nose_semantics::PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID
        ))
    );

    let py_iter_shadowed = lower_fixture(
        "typing_iter_alias_shadowed.py",
        b"from typing import Iterable as I\nI = object\ndef f(xs: I[int]):\n    return xs\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&py_iter_shadowed.evidence, DomainEvidence::Iterable),
        0,
        "a rebound iterable alias must not emit parameter Domain evidence"
    );

    let py_iter_class_shadowed = lower_fixture(
            "typing_iter_alias_class_shadowed.py",
            b"from typing import Iterable as I\nclass I:\n    pass\ndef f(xs: I[int]):\n    return xs\n",
            Lang::Python,
            interner,
        );
    assert_eq!(
        param_domain_record_count(&py_iter_class_shadowed.evidence, DomainEvidence::Iterable),
        0,
        "a class definition with the alias name must close later Domain evidence"
    );

    let py_iter_function_shadowed = lower_fixture(
            "typing_iter_alias_function_shadowed.py",
            b"from typing import Iterable as I\ndef I():\n    return None\ndef f(xs: I[int]):\n    return xs\n",
            Lang::Python,
            interner,
        );
    assert_eq!(
        param_domain_record_count(
            &py_iter_function_shadowed.evidence,
            DomainEvidence::Iterable
        ),
        0,
        "a function definition with the alias name must close later Domain evidence"
    );
}

fn assert_python_stdlib_pack_param_domains(interner: &Interner) {
    let py_mapping_alias = lower_fixture(
        "collections_abc_mapping_alias.py",
        b"from collections.abc import Mapping as M\ndef f(xs: M[str, int]):\n    return xs\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        param_domain_record_count_from_pack(
            &py_mapping_alias.evidence,
            DomainEvidence::Map,
            nose_semantics::PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID
        ),
        1,
        "collections.abc aliases should resolve through the same pilot pack"
    );

    let py_future_alias = lower_fixture(
        "asyncio_future_alias.py",
        b"from asyncio import Future as Fut\ndef f(x: Fut[int]):\n    return x\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        param_domain_record_count_from_pack(
            &py_future_alias.evidence,
            DomainEvidence::FutureLike,
            nose_semantics::PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID
        ),
        1,
        "asyncio Future aliases should resolve through the same pilot pack"
    );

    let py_shadowed = lower_fixture(
        "typing_alias_shadowed.py",
        b"from typing import List as L\nL = object\ndef f(xs: L[int]):\n    return xs\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&py_shadowed.evidence, DomainEvidence::Collection),
        0,
        "a rebound typing alias must not emit parameter Domain evidence"
    );

    let py_wrong_module_alias = lower_fixture(
        "typing_alias_wrong_module.py",
        b"from project.typing import Iterable as I\ndef f(xs: I[int]):\n    return xs\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&py_wrong_module_alias.evidence, DomainEvidence::Iterable),
        0,
        "a same-named alias from another module must not satisfy the stdlib pack"
    );
}

fn assert_ts_and_java_param_domains(interner: &Interner) {
    let ts = lower_fixture(
            "domain_types.ts",
            b"function f(a: Bitmap<string, number>, b: Blacklist<string>, c: string[], d: Set<string>) { return c.length; }\n",
            Lang::TypeScript,
            interner,
        );
    assert_eq!(
        param_domain_record_count(&ts.evidence, DomainEvidence::Map),
        0,
        "Bitmap must not be treated as Map by substring"
    );
    assert_eq!(
        param_domain_record_count(&ts.evidence, DomainEvidence::Collection),
        0,
        "Blacklist must not be treated as Collection by substring"
    );
    assert_eq!(
        param_domain_record_count(&ts.evidence, DomainEvidence::Array),
        1,
        "string[] should still emit array domain evidence"
    );
    assert_eq!(
        param_domain_record_count(&ts.evidence, DomainEvidence::Set),
        1,
        "Set<T> should still emit set domain evidence"
    );

    let ts_rich = lower_fixture(
            "domain_types_rich.ts",
            b"function f(a: Iterable<string>, b: Iterator<string>, c: Promise<string>, d: Record<string, number>, e: Result<string, Error>, f: boolean) { return f; }\n",
            Lang::TypeScript,
            interner,
        );
    assert_eq!(
        param_domain_record_count(&ts_rich.evidence, DomainEvidence::Iterable),
        1
    );
    assert_eq!(
        param_domain_record_count(&ts_rich.evidence, DomainEvidence::Iterator),
        1
    );
    assert_eq!(
        param_domain_record_count(&ts_rich.evidence, DomainEvidence::PromiseLike),
        1
    );
    assert_eq!(
        param_domain_record_count(&ts_rich.evidence, DomainEvidence::Record),
        1
    );
    assert_eq!(
        param_domain_record_count(&ts_rich.evidence, DomainEvidence::Result),
        1
    );
    assert_eq!(
        param_domain_record_count(&ts_rich.evidence, DomainEvidence::Boolean),
        1
    );

    let java = lower_fixture(
        "Annotated.java",
        b"class T { void f(@Ann(\"...\") String value, @Nonnull List<String> xs) {} }\n",
        Lang::Java,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&java.evidence, DomainEvidence::Array),
        0,
        "annotation strings containing ... must not imply Java array/varargs domain"
    );
    assert_eq!(
        param_domain_record_count(&java.evidence, DomainEvidence::String),
        1
    );
    assert_eq!(
        param_domain_record_count(&java.evidence, DomainEvidence::Collection),
        1
    );
}

fn assert_rust_result_param_domains(interner: &Interner) {
    let rust_result = lower_fixture(
        "result_param.rs",
        b"pub fn f(value: Result<i32, i32>) -> bool { value.is_ok() }\n",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&rust_result.evidence, DomainEvidence::Result),
        1,
        "unshadowed Rust Result<T, E> should still emit parameter domain evidence"
    );

    let rust_qualified_result = lower_fixture(
        "qualified_result_param.rs",
        b"struct Result<T, E> { value: T, err: E }\npub fn f(value: std::result::Result<i32, i32>) -> bool { value.is_ok() }\n",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&rust_qualified_result.evidence, DomainEvidence::Result),
        1,
        "qualified std::result::Result should not be blocked by a local Result type"
    );

    let rust_shadowed_result = lower_fixture(
        "shadowed_result_param.rs",
        b"struct Result<T, E> { value: T, err: E }\npub fn f(value: Result<i32, i32>) -> bool { value.is_ok() }\n",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        param_domain_record_count(&rust_shadowed_result.evidence, DomainEvidence::Result),
        0,
        "a local Rust Result type must close unqualified std Result parameter evidence"
    );
}
