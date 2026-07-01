use super::*;

#[test]
fn java_future_executor_type_domains_are_import_backed() {
    let interner = Interner::new();

    let java_future = lower_fixture(
        "FutureDomain.java",
        b"import java.util.concurrent.CompletableFuture;\nclass T { void f(CompletableFuture<String> future) {} }\n",
        Lang::Java,
        &interner,
    );
    let future_import_ids = imported_binding_symbol_ids(
        &java_future.evidence,
        "java.util.concurrent",
        "CompletableFuture",
    );
    assert_eq!(future_import_ids.len(), 1);
    let future_domains = param_domain_records(&java_future.evidence, DomainEvidence::FutureLike);
    assert_eq!(future_domains.len(), 1);
    assert_eq!(
        future_domains[0].dependencies, future_import_ids,
        "Java exact-imported Future-like parameter domains should be import-backed"
    );

    let java_future_handle = lower_fixture(
        "FutureHandleDomain.java",
        b"import java.util.concurrent.Future;\nclass T { void f(Future<String> future) {} }\n",
        Lang::Java,
        &interner,
    );
    let future_handle_import_ids = imported_binding_symbol_ids(
        &java_future_handle.evidence,
        "java.util.concurrent",
        "Future",
    );
    assert_eq!(future_handle_import_ids.len(), 1);
    let future_handle_domains =
        param_domain_records(&java_future_handle.evidence, DomainEvidence::FutureLike);
    assert_eq!(future_handle_domains.len(), 1);
    assert_eq!(
        future_handle_domains[0].dependencies, future_handle_import_ids,
        "Java exact-imported Future handle domains should be import-backed separately from CompletionStage"
    );

    let java_wildcard_future = lower_fixture(
        "WildcardFutureDomain.java",
        b"import java.util.concurrent.*;\nclass T { void f(Future<String> future) {} }\n",
        Lang::Java,
        &interner,
    );
    let wildcard_future_import_ids = imported_binding_symbol_ids(
        &java_wildcard_future.evidence,
        "java.util.concurrent",
        "Future",
    );
    assert_eq!(wildcard_future_import_ids.len(), 1);
    let wildcard_future_domains =
        param_domain_records(&java_wildcard_future.evidence, DomainEvidence::FutureLike);
    assert_eq!(wildcard_future_domains.len(), 1);
    assert_eq!(
        wildcard_future_domains[0].dependencies, wildcard_future_import_ids,
        "Java wildcard-imported Future parameter domains should be backed by wildcard-derived import evidence"
    );

    let java_future_conflict = lower_fixture(
        "FutureConflictDomain.java",
        b"import java.util.concurrent.Future;\nimport example.Future;\nclass T { void f(Future<String> future) {} }\n",
        Lang::Java,
        &interner,
    );
    let conflicted_future_import_ids = imported_binding_symbol_ids(
        &java_future_conflict.evidence,
        "java.util.concurrent",
        "Future",
    );
    assert_eq!(conflicted_future_import_ids.len(), 1);
    let conflicted_future_domains =
        param_domain_records(&java_future_conflict.evidence, DomainEvidence::FutureLike);
    assert!(
        conflicted_future_domains
            .iter()
            .all(|record| record.dependencies != conflicted_future_import_ids),
        "a conflicting Java Future import must clear stale import-backed FutureLike aliases"
    );

    let executor_domain = DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("java.util.concurrent.ExecutorService"),
    };
    let java_executor = lower_fixture(
        "ExecutorDomain.java",
        b"import java.util.concurrent.ExecutorService;\nclass T { void f(ExecutorService executor) {} }\n",
        Lang::Java,
        &interner,
    );
    let executor_import_ids = imported_binding_symbol_ids(
        &java_executor.evidence,
        "java.util.concurrent",
        "ExecutorService",
    );
    assert_eq!(executor_import_ids.len(), 1);
    let executor_domains = param_domain_records(&java_executor.evidence, executor_domain);
    assert_eq!(executor_domains.len(), 1);
    assert_eq!(
        executor_domains[0].dependencies, executor_import_ids,
        "Java exact-imported ExecutorService receiver domains should retain import evidence"
    );

    let java_wildcard_executor = lower_fixture(
        "WildcardExecutorDomain.java",
        b"import java.util.concurrent.*;\nclass T { void f(ExecutorService executor) {} }\n",
        Lang::Java,
        &interner,
    );
    let wildcard_executor_import_ids = imported_binding_symbol_ids(
        &java_wildcard_executor.evidence,
        "java.util.concurrent",
        "ExecutorService",
    );
    assert_eq!(wildcard_executor_import_ids.len(), 1);
    let wildcard_executor_domains =
        param_domain_records(&java_wildcard_executor.evidence, executor_domain);
    assert_eq!(wildcard_executor_domains.len(), 1);
    assert_eq!(
        wildcard_executor_domains[0].dependencies, wildcard_executor_import_ids,
        "Java wildcard-imported ExecutorService receiver domains should retain wildcard-derived import evidence"
    );

    let java_future_bindings = lower_fixture(
        "FutureBindingDomain.java",
        b"import java.util.concurrent.Future;\nclass T { private Future<String> field; void f() { Future<String> local = make(); } }\n",
        Lang::Java,
        &interner,
    );
    let future_binding_import_ids = imported_binding_symbol_ids(
        &java_future_bindings.evidence,
        "java.util.concurrent",
        "Future",
    );
    assert_eq!(future_binding_import_ids.len(), 1);
    let future_binding_domains =
        binding_domain_records(&java_future_bindings.evidence, DomainEvidence::FutureLike);
    assert_eq!(future_binding_domains.len(), 2);
    assert!(future_binding_domains
        .iter()
        .all(|record| record.dependencies == future_binding_import_ids));

    let java_executor_binding = lower_fixture(
        "ExecutorBindingDomain.java",
        b"import java.util.concurrent.ExecutorService;\nclass T { void f() { ExecutorService local = make(); local.submit(() -> work()); } }\n",
        Lang::Java,
        &interner,
    );
    let executor_binding_import_ids = imported_binding_symbol_ids(
        &java_executor_binding.evidence,
        "java.util.concurrent",
        "ExecutorService",
    );
    let executor_binding_domains =
        binding_domain_records(&java_executor_binding.evidence, executor_domain);
    assert_eq!(executor_binding_domains.len(), 1);
    assert_eq!(
        executor_binding_domains[0].dependencies,
        executor_binding_import_ids
    );

    let java_wildcard_future_bindings = lower_fixture(
        "WildcardFutureBindingDomain.java",
        b"import java.util.concurrent.*;\nclass T { private Future<String> field; void f() { Future<String> local = make(); } }\n",
        Lang::Java,
        &interner,
    );
    let wildcard_future_binding_import_ids = imported_binding_symbol_ids(
        &java_wildcard_future_bindings.evidence,
        "java.util.concurrent",
        "Future",
    );
    assert_eq!(wildcard_future_binding_import_ids.len(), 1);
    let wildcard_future_binding_domains = binding_domain_records(
        &java_wildcard_future_bindings.evidence,
        DomainEvidence::FutureLike,
    );
    assert_eq!(wildcard_future_binding_domains.len(), 2);
    assert!(wildcard_future_binding_domains
        .iter()
        .all(|record| record.dependencies == wildcard_future_binding_import_ids));
}
