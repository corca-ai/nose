use super::support::{
    java_provider_and_importer_src, remove_library_api_evidence_by_rule, resolve_snapshot_count,
};
use nose_il::{FileId, Interner, Lang};
use nose_semantics::{
    JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
    PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
};

#[test]
fn java_collection_provider_requires_library_api_evidence_for_snapshot() {
    let interner = Interner::new();
    let provider_src = "import java.util.List;\nclass Tables { static final List<String> VALUES = List.of(\"red\", \"blue\"); }\n";
    let importer_src = "import static Tables.VALUES;\nclass Consumer { static boolean member(String value, String other) { return VALUES.contains(value); } }\n";
    let (provider, importer) =
        java_provider_and_importer_src(provider_src, importer_src, &interner);
    assert_eq!(
        resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
        1
    );

    let mut missing_api = provider;
    remove_library_api_evidence_by_rule(
        &mut missing_api,
        JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    );
    assert_eq!(
        resolve_snapshot_count(missing_api, importer, &interner),
        0,
        "Java import/symbol proof must not prove provider List.of without LibraryApi evidence"
    );
}

#[test]
fn java_collection_provider_rejects_single_arg_arrays_aslist_snapshot() {
    let interner = Interner::new();
    let provider_src = "import java.util.Arrays;\nclass Tables { static final java.util.List<String> VALUES = Arrays.asList(\"red\"); }\n";
    let importer_src = "import static Tables.VALUES;\nclass Consumer { static boolean member(String value, String other) { return VALUES.contains(value); } }\n";
    let (provider, importer) =
        java_provider_and_importer_src(provider_src, importer_src, &interner);

    assert_eq!(
        resolve_snapshot_count(provider, importer, &interner),
        0,
        "single-argument Arrays.asList stays closed at the imported snapshot boundary"
    );
}

#[test]
fn python_builtin_collection_provider_requires_library_api_evidence_for_snapshot() {
    let interner = Interner::new();
    let provider = lower_python(
        FileId(0),
        "tables.py",
        "VALUES = set([\"red\", \"blue\"])\n",
        &interner,
    );
    let importer = lower_python(
        FileId(1),
        "consumer.py",
        "from tables import VALUES\n\ndef member(value, other):\n    return value in VALUES\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
        1
    );

    let mut missing_api = provider;
    remove_library_api_evidence_by_rule(
        &mut missing_api,
        PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
    );
    assert_eq!(
        resolve_snapshot_count(missing_api, importer, &interner),
        0,
        "Python import proof must not prove provider set(...) without LibraryApi evidence"
    );
}

#[test]
fn python_imported_collection_provider_requires_library_api_evidence_for_snapshot() {
    let interner = Interner::new();
    let provider = lower_python(
        FileId(0),
        "tables.py",
        "from collections import deque\nVALUES = deque([\"red\", \"blue\"])\n",
        &interner,
    );
    let importer = lower_python(
        FileId(1),
        "consumer.py",
        "from tables import VALUES\n\ndef member(value, other):\n    return VALUES.__contains__(value)\n",
        &interner,
    );
    assert_eq!(
        resolve_snapshot_count(provider.clone(), importer.clone(), &interner),
        1
    );

    let mut missing_api = provider;
    remove_library_api_evidence_by_rule(
        &mut missing_api,
        PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    );
    assert_eq!(
        resolve_snapshot_count(missing_api, importer, &interner),
        0,
        "Python import proof must not prove provider collections.deque without LibraryApi evidence"
    );
}

#[test]
fn python_collection_provider_rejects_shadowed_builtin_factory() {
    let interner = Interner::new();
    let provider = lower_python(
        FileId(0),
        "tables.py",
        "def set(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\nVALUES = set([\"red\", \"blue\"])\n",
        &interner,
    );
    let importer = lower_python(
        FileId(1),
        "consumer.py",
        "from tables import VALUES\n\ndef member(value, other):\n    return value in VALUES\n",
        &interner,
    );

    assert_eq!(
        resolve_snapshot_count(provider, importer, &interner),
        0,
        "provider-local set shadowing must block imported collection snapshots"
    );
}

fn lower_python(file: FileId, path: &str, src: &str, interner: &Interner) -> nose_il::Il {
    crate::lower_source(file, path, src.as_bytes(), Lang::Python, interner)
        .expect("lower Python source")
}
