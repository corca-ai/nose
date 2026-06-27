use super::*;

#[test]
fn collection_membership_converges_with_python_imported_collection_factories() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_python_collection_membership",
        &[
            (
                "literal.py",
                "def member(value, other):\n    return value in [\"red\", \"blue\"]\n",
            ),
            (
                "set_tables.py",
                "VALUES = set([\"red\", \"blue\"])\n",
            ),
            (
                "set_imported.py",
                "from set_tables import VALUES\n\ndef member(value, other):\n    return value in VALUES\n",
            ),
            (
                "deque_tables.py",
                "from collections import deque\nVALUES = deque([\"red\", \"blue\"])\n",
            ),
            (
                "deque_imported.py",
                "from deque_tables import VALUES\n\ndef member(value, other):\n    return VALUES.__contains__(value)\n",
            ),
            (
                "mutated_tables.py",
                "VALUES = set([\"red\", \"blue\"])\nVALUES.add(\"green\")\n",
            ),
            (
                "imported_mutated_provider.py",
                "from mutated_tables import VALUES\n\ndef member(value, other):\n    return value in VALUES\n",
            ),
            (
                "imported_mutated_receiver.py",
                "from set_tables import VALUES\nVALUES.add(\"green\")\n\ndef member(value, other):\n    return value in VALUES\n",
            ),
            (
                "wrong_collection.py",
                "from set_tables import VALUES\n\ndef member(value, other):\n    return value in set([\"green\", \"blue\"])\n",
            ),
            (
                "shadowed_tables.py",
                "def set(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\nVALUES = set([\"red\", \"blue\"])\n",
            ),
            (
                "imported_shadowed_provider.py",
                "from shadowed_tables import VALUES\n\ndef member(value, other):\n    return value in VALUES\n",
            ),
        ],
    );
    let literal = corpus_value_fp(&corpus, "literal.py", "member");
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "set_imported.py", "member"),
        "imported Python set(...) binding should retain provider factory provenance"
    );
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "deque_imported.py", "member"),
        "imported Python deque(...) binding should retain provider factory provenance"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "imported_mutated_provider.py", "member"),
        "provider mutation must block Python imported collection provenance"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "imported_mutated_receiver.py", "member"),
        "importer mutation must keep Python imported collection receiver distinct"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "wrong_collection.py", "member"),
        "different imported collection contents must stay distinct"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "imported_shadowed_provider.py", "member"),
        "provider-local Python factory shadowing must block imported collection provenance"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn collection_membership_converges_with_java_imported_collection_factories() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_java_collection_membership",
        &[
            (
                "literal.py",
                "def member(value, other):\n    return value in [\"red\", \"blue\"]\n",
            ),
            (
                "Tables.java",
                "import java.util.List;\nclass Tables { static final List<String> VALUES = List.of(\"red\", \"blue\"); }\n",
            ),
            (
                "JavaImported.java",
                "import static Tables.VALUES;\nclass JavaImported { static boolean member(String value, String other) { return VALUES.contains(value); } }\n",
            ),
            (
                "SetTables.java",
                "import java.util.Set;\nclass SetTables { static final Set<String> VALUES = Set.of(\"red\", \"blue\"); }\n",
            ),
            (
                "JavaImportedSet.java",
                "import static SetTables.VALUES;\nclass JavaImportedSet { static boolean member(String value, String other) { return VALUES.contains(value); } }\n",
            ),
            (
                "WrongTables.java",
                "import java.util.List;\nclass WrongTables { static final List<String> VALUES = List.of(\"green\", \"blue\"); }\n",
            ),
            (
                "JavaImportedWrongCollection.java",
                "import static WrongTables.VALUES;\nclass JavaImportedWrongCollection { static boolean member(String value, String other) { return VALUES.contains(value); } }\n",
            ),
            (
                "MissingImportTables.java",
                "class MissingImportTables { static final List<String> VALUES = List.of(\"red\", \"blue\"); }\nclass List<T> { static Box of(String left, String right) { return new Box(); } }\nclass Box { boolean contains(String value) { return false; } }\n",
            ),
            (
                "JavaImportedMissingImport.java",
                "import static MissingImportTables.VALUES;\nclass JavaImportedMissingImport { static boolean member(String value, String other) { return VALUES.contains(value); } }\n",
            ),
            (
                "MutatedTables.java",
                "import java.util.List;\nclass MutatedTables { static final List<String> VALUES = List.of(\"red\", \"blue\"); static { VALUES.add(\"green\"); } }\n",
            ),
            (
                "JavaImportedMutatedProvider.java",
                "import static MutatedTables.VALUES;\nclass JavaImportedMutatedProvider { static boolean member(String value, String other) { return VALUES.contains(value); } }\n",
            ),
            (
                "JavaImportedMutatedReceiver.java",
                "import static Tables.VALUES;\nclass JavaImportedMutatedReceiver { static boolean member(String value, String other) { VALUES.add(\"green\"); return VALUES.contains(value); } }\n",
            ),
        ],
    );
    let literal = corpus_value_fp(&corpus, "literal.py", "member");
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "JavaImported.java", "member"),
        "Java static-imported List.of binding should retain provider factory provenance"
    );
    assert_eq!(
        literal,
        corpus_value_fp(&corpus, "JavaImportedSet.java", "member"),
        "Java static-imported Set.of binding should retain provider factory provenance"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "JavaImportedWrongCollection.java", "member"),
        "different Java imported collection contents must stay distinct"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "JavaImportedMissingImport.java", "member"),
        "Java provider factory proof must require java.util import evidence"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "JavaImportedMutatedProvider.java", "member"),
        "provider mutation must block Java imported collection provenance"
    );
    assert_ne!(
        literal,
        corpus_value_fp(&corpus, "JavaImportedMutatedReceiver.java", "member"),
        "importer mutation must keep Java imported collection receiver distinct"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
