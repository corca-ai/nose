use super::*;
use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceKind, EvidenceStatus, Lang, LibraryApiEvidenceKind,
    SymbolEvidenceKind, TypeEvidenceKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash,
    library_free_name_collection_factory_contract, library_method_call_contract,
    library_swift_map_factory_contract, LibraryApiCalleeContract, LibraryApiContractId,
};
use std::fs;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_frontend_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn discover_paths_accepts_direct_supported_file() {
    let dir = temp_dir("direct_supported_file");
    let file = dir.join("sample.py");
    fs::write(&file, "def f():\n    return 1\n").unwrap();

    let paths = discover_paths(&file, &[]);

    assert_eq!(
        paths,
        vec![(file.to_string_lossy().to_string(), Lang::Python)]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn discover_paths_ignores_direct_unsupported_file() {
    let dir = temp_dir("direct_unsupported_file");
    let file = dir.join("README.txt");
    fs::write(&file, "not source\n").unwrap();

    assert!(discover_paths(&file, &[]).is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn discover_unique_paths_keeps_hard_links_distinct() {
    let dir = temp_dir("hard_links");
    let original = dir.join("original.py");
    let linked = dir.join("linked.py");
    fs::write(&original, "def f():\n    return 1\n").unwrap();
    fs::hard_link(&original, &linked).unwrap();

    let paths = discover_unique_paths(&[dir.as_path()], &[]);
    let files = paths
        .iter()
        .map(|(path, _lang)| Path::new(path).file_name().unwrap().to_string_lossy())
        .collect::<Vec<_>>();

    assert_eq!(paths.len(), 2, "hard-linked source files remain distinct");
    assert!(files.iter().any(|file| file == "original.py"));
    assert!(files.iter().any(|file| file == "linked.py"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lower_corpus_skips_ansi_highlight_artifacts() {
    let dir = temp_dir("ansi_highlight_artifacts");
    let source = dir.join("keep.go");
    let highlighted = dir.join("tests/syntax-tests/highlighted/Go/main.go");
    fs::create_dir_all(highlighted.parent().unwrap()).unwrap();
    fs::write(&source, "package main\nfunc keep() int { return 1 }\n").unwrap();
    fs::write(
        &highlighted,
        b"\x1b[38;2;1;2;3mfunc\x1b[0m \x1b[38;2;4;5;6mnope\x1b[0m() {}\n",
    )
    .unwrap();

    let corpus = lower_corpus_filtered(&[dir.as_path()], &[]);
    let paths: Vec<_> = corpus
        .files
        .iter()
        .map(|il| il.meta.path.as_str())
        .collect();

    assert!(paths.iter().any(|path| path.ends_with("keep.go")));
    assert!(
        paths
            .iter()
            .all(|path| !path.ends_with("tests/syntax-tests/highlighted/Go/main.go")),
        "highlighted ANSI output must not be parsed as Go source: {paths:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lower_corpus_skips_binary_source_artifacts() {
    let dir = temp_dir("binary_source_artifacts");
    let source = dir.join("keep.js");
    let fake_source = dir.join("media/testdata/fake.js");
    fs::create_dir_all(fake_source.parent().unwrap()).unwrap();
    fs::write(&source, "export function keep() { return 1; }\n").unwrap();
    fs::write(
        &fake_source,
        b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01",
    )
    .unwrap();

    let corpus = lower_corpus_filtered(&[dir.as_path()], &[]);
    let paths: Vec<_> = corpus
        .files
        .iter()
        .map(|il| il.meta.path.as_str())
        .collect();

    assert!(paths.iter().any(|path| path.ends_with("keep.js")));
    assert!(
        paths.iter().all(|path| !path.ends_with("fake.js")),
        "binary files with source extensions must not be parsed as source: {paths:?}"
    );
    assert_eq!(
        source_artifacts::skip_reason(
            &fake_source,
            Lang::JavaScript,
            fs::read(&fake_source).unwrap().as_slice()
        ),
        Some("binary-source-artifact")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lower_corpus_skips_obvious_cpp_headers_routed_as_c() {
    let dir = temp_dir("cpp_header_routing");
    let c_header = dir.join("api.h");
    let cpp_header = dir.join("runtime/Cpp/runtime/src/Stream.h");
    fs::create_dir_all(cpp_header.parent().unwrap()).unwrap();
    fs::write(
        &c_header,
        "/* namespace fake { class NotCode { public: }; } */\n#pragma once\nint add(int a, int b);\n",
    )
    .unwrap();
    fs::write(
        &cpp_header,
        "#pragma once\nnamespace antlr4 {\nclass Stream {\npublic:\n  virtual void load();\n};\n}\n",
    )
    .unwrap();

    let corpus = lower_corpus_filtered(&[dir.as_path()], &[]);
    let paths: Vec<_> = corpus
        .files
        .iter()
        .map(|il| il.meta.path.as_str())
        .collect();

    assert!(paths.iter().any(|path| path.ends_with("api.h")));
    assert!(
        paths.iter().all(|path| !path.ends_with("Stream.h")),
        "unsupported C++ headers must not be parsed as C source: {paths:?}"
    );
    assert_eq!(
        source_artifacts::skip_reason(&c_header, Lang::C, fs::read(&c_header).unwrap().as_slice()),
        None
    );
    assert_eq!(
        source_artifacts::skip_reason(
            &cpp_header,
            Lang::C,
            fs::read(&cpp_header).unwrap().as_slice()
        ),
        Some("unsupported-cpp-header")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lower_corpus_closes_swift_stdlib_factories_shadowed_by_cross_file_typealias() {
    let dir = temp_dir("swift_cross_file_typealias_shadow");
    fs::write(
        dir.join("A.swift"),
        r#"struct MyArray {
  init(_ values: [Int]) {}
}
struct MyDictionary {
  init(uniqueKeysWithValues values: [(String, Int)]) {}
}
struct MySet {
  init(_ values: [Int]) {}
}
typealias Array = MyArray
typealias Set = MySet
typealias Dictionary = MyDictionary
"#,
    )
    .unwrap();
    let consumer = dir.join("B.swift");
    fs::write(
        &consumer,
        r#"func f(values: [Int]) {
  _ = Array(values)
  _ = Set(values)
  _ = Dictionary(uniqueKeysWithValues: [("a", 1)])
}
"#,
    )
    .unwrap();

    let corpus = lower_corpus_filtered(&[dir.as_path()], &[]);
    let consumer_il = corpus
        .files
        .iter()
        .find(|il| il.meta.path == consumer.to_string_lossy())
        .expect("consumer Swift file should be lowered");
    let array_contract = library_free_name_collection_factory_contract(Lang::Swift, "Array")
        .expect("Swift Array contract");
    let set_contract = library_free_name_collection_factory_contract(Lang::Swift, "Set")
        .expect("Swift Set contract");
    let dictionary_contract =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .expect("Swift Dictionary contract");

    assert_eq!(
        asserted_contract_api_count(consumer_il, array_contract.id, array_contract.callee),
        0,
        "cross-file typealias Array must close stdlib Array(sequence) API evidence"
    );
    assert_eq!(
        asserted_contract_api_count(consumer_il, set_contract.id, set_contract.callee),
        0,
        "cross-file typealias Set must close stdlib Set(sequence) API evidence"
    );
    assert_eq!(
        asserted_contract_api_count(
            consumer_il,
            dictionary_contract.id,
            dictionary_contract.callee
        ),
        0,
        "cross-file typealias Dictionary must close stdlib Dictionary API evidence"
    );
    assert_eq!(
        asserted_domain_count(consumer_il, DomainEvidence::Array),
        0,
        "Array(sequence) result-domain proof must depend on the closed API proof"
    );
    assert_eq!(
        asserted_domain_count(consumer_il, DomainEvidence::Set),
        0,
        "Set(sequence) result-domain proof must depend on the closed API proof"
    );
    assert_eq!(
        asserted_domain_count(consumer_il, DomainEvidence::Map),
        0,
        "Dictionary result-domain proof must depend on the closed API proof"
    );
    assert_eq!(
        asserted_unshadowed_global_count(consumer_il, "Array")
            + asserted_unshadowed_global_count(consumer_il, "Set")
            + asserted_unshadowed_global_count(consumer_il, "Dictionary"),
        0,
        "cross-file stdlib type shadows must close unshadowed-global proofs"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lower_corpus_closes_cross_file_custom_dictionary_default_subscript() {
    let dir = temp_dir("swift_cross_file_dictionary_default_subscript");
    fs::write(
        dir.join("Custom.swift"),
        r#"struct Dictionary<Key: Hashable, Value> {
  subscript(key: Key, default fallback: Value) -> Value { fallback }
}
"#,
    )
    .unwrap();
    let consumer = dir.join("Consumer.swift");
    fs::write(
        &consumer,
        r#"func lookup(_ table: Dictionary<String, Int>, _ key: String, _ fallback: Int) -> Int {
  table[key, default: fallback]
}
"#,
    )
    .unwrap();

    let corpus = lower_corpus_filtered(&[dir.as_path()], &[]);
    let consumer_il = corpus
        .files
        .iter()
        .find(|il| il.meta.path == consumer.to_string_lossy())
        .expect("consumer Swift file should be lowered");
    assert!(consumer_il.evidence.iter().any(|record| {
        record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter)
            && record.status == EvidenceStatus::Ambiguous
    }));
    assert!(
        !consumer_il.evidence.iter().any(|record| {
            record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter)
                && record.status == EvidenceStatus::Asserted
        }),
        "cross-file custom Dictionary must tombstone the stdlib receiver proof"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lower_corpus_closes_swift_compact_map_shadowed_by_cross_file_overload() {
    assert_cross_file_swift_compact_map_closed(
        "swift_cross_file_compact_map_overload",
        "Overload.swift",
        r#"extension Array where Element == Bool {
  func `compactMap`(_ transform: (Bool) -> Bool?) -> [Bool] { return [] }
}
"#,
        "a cross-file Array.compactMap overload must close stdlib HOF evidence",
    );
}

#[test]
fn lower_corpus_closes_swift_compact_map_shadowed_by_cross_file_property() {
    assert_cross_file_swift_compact_map_closed(
        "swift_cross_file_compact_map_property",
        "Property.swift",
        r#"extension Array where Element==Bool{var `compactMap`:(((Bool)->Bool?)->[Bool]){{_ in []}}}
"#,
        "a cross-file compactMap property must close stdlib HOF evidence",
    );
}

#[test]
fn lower_corpus_closes_compact_map_against_custom_filter_map_dispatch() {
    assert_cross_file_swift_compact_map_closed(
        "swift_cross_file_filter_map_dispatch",
        "Overrides.swift",
        r#"extension Array where Element == Bool {
  func filter(_ predicate: (Bool) -> Bool) -> [Bool] { [] }
  func map<T>(_ transform: (Bool) -> T) -> [T] { [] }
}
"#,
        "custom filter/map dispatch must close compactMap graph convergence",
    );
}

#[test]
fn lower_corpus_closes_swift_compact_map_after_cross_file_nil_conformance() {
    assert_cross_file_swift_compact_map_closed(
        "swift_cross_file_nil_literal_conformance",
        "Conformance.swift",
        r#"typealias NilProtocol = ExpressibleByNilLiteral
extension Bool: @retroactive NilProtocol {
  public init(nilLiteral: ()) { self = true }
}
"#,
        "a cross-file nil-literal conformance must close compactMap option-channel proof",
    );
}

#[test]
fn lower_corpus_closes_swift_flat_map_shadowed_by_cross_file_overload() {
    assert_cross_file_swift_flat_map_closed(
        "swift_cross_file_flat_map_overload",
        r#"extension Array where Element == [Bool] {
  func flatMap<T>(_ transform: ([Bool]) -> [T]) -> [T] { [] }
}
"#,
        "a cross-file Array.flatMap overload must close stdlib HOF evidence",
    );
}

#[test]
fn lower_corpus_closes_swift_flat_map_after_cross_file_inner_map_overload() {
    assert_cross_file_swift_flat_map_closed(
        "swift_cross_file_flat_map_inner_map_overload",
        r#"extension Array where Element == Bool {
  func map<T>(_ transform: (Bool) -> T) -> [T] { [] }
}
"#,
        "a cross-file inner Array.map overload must close one-level flatMap evidence",
    );
}

#[test]
fn lower_corpus_closes_guarded_swift_flat_map_after_cross_file_filter_overload() {
    assert_cross_file_swift_method_contract_count(
        "swift_cross_file_flat_map_filter_overload",
        "Overload.swift",
        r#"extension Array where Element == Int {
  func filter(_ predicate: (Int) -> Bool) -> [Int] { [] }
}
"#,
        r#"func f(_ groups: [[Int]], _ enabled: Bool) -> [Int] {
  return groups.filter { group in enabled }.flatMap { (group: [Int]) in
    group.filter { value in enabled }.map { value in value }
  }
}
"#,
        "flatMap",
        0,
        "a cross-file Array.filter overload must close guarded flatMap evidence",
    );
}

#[test]
fn lower_corpus_closes_swift_all_satisfy_after_cross_file_overload() {
    assert_cross_file_swift_method_contract_count(
        "swift_cross_file_all_satisfy_overload",
        "Overload.swift",
        r#"extension Array where Element == Int {
  func allSatisfy(_ predicate: (Int) -> Bool) -> Bool { false }
}
"#,
        r#"func f(_ groups: [[Int]], _ minimum: Int) -> Bool {
  return groups.flatMap { (group: [Int]) in group.map { value in value } }
    .allSatisfy { value in value >= minimum }
}
"#,
        "allSatisfy",
        0,
        "a cross-file Array.allSatisfy overload must close terminal aggregate evidence",
    );
}

#[test]
fn lower_corpus_keeps_swift_all_satisfy_after_disjoint_callback_arity_overload() {
    assert_cross_file_swift_method_contract_count(
        "swift_cross_file_all_satisfy_disjoint_overload",
        "Overload.swift",
        r#"extension Array {
  func allSatisfy(_ predicate: (Element, Int) -> Bool) -> Bool { false }
}
"#,
        r#"func f(_ values: [Int]) -> Bool {
  return values.allSatisfy { value in value >= 0 }
}
"#,
        "allSatisfy",
        1,
        "a disjoint two-argument callback overload must not close standard unary allSatisfy evidence",
    );
}

fn assert_cross_file_swift_flat_map_closed(tag: &str, declaration: &str, failure_message: &str) {
    assert_cross_file_swift_method_contract_count(
        tag,
        "Overload.swift",
        declaration,
        r#"func f(_ groups: [[Bool]]) -> [Bool] {
  return groups.flatMap { (group: [Bool]) in group.map { value in value } }
}
"#,
        "flatMap",
        0,
        failure_message,
    );
}

fn assert_cross_file_swift_compact_map_closed(
    tag: &str,
    declaration_file: &str,
    declaration: &str,
    failure_message: &str,
) {
    assert_cross_file_swift_method_contract_count(
        tag,
        declaration_file,
        declaration,
        r#"func f(_ values: [Bool]) -> [Bool] {
  return values.compactMap { value in value ? value : nil }
}
"#,
        "compactMap",
        0,
        failure_message,
    );
}

fn assert_cross_file_swift_method_contract_count(
    tag: &str,
    declaration_file: &str,
    declaration: &str,
    consumer_source: &str,
    method: &str,
    expected_count: usize,
    failure_message: &str,
) {
    let dir = temp_dir(tag);
    fs::write(dir.join(declaration_file), declaration).unwrap();
    let consumer = dir.join("Consumer.swift");
    fs::write(&consumer, consumer_source).unwrap();

    let corpus = lower_corpus_filtered(&[dir.as_path()], &[]);
    let consumer_il = corpus
        .files
        .iter()
        .find(|il| il.meta.path == consumer.to_string_lossy())
        .expect("consumer Swift file should be lowered");
    let contract = library_method_call_contract(Lang::Swift, method, 1)
        .unwrap_or_else(|| panic!("{method} contract"));
    assert_eq!(
        asserted_contract_api_count(consumer_il, contract.id, contract.callee),
        expected_count,
        "{failure_message}"
    );
    let _ = fs::remove_dir_all(&dir);
}

fn asserted_contract_api_count(
    il: &Il,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> usize {
    let contract_hash = library_api_contract_id_hash(id);
    let callee_hash = library_api_callee_contract_hash(callee);
    il.evidence
        .iter()
        .filter(|record| {
            record.status == EvidenceStatus::Asserted
                && matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        ..
                    }) if actual_contract == contract_hash && actual_callee == callee_hash
                )
        })
        .count()
}

fn asserted_domain_count(il: &Il, expected: DomainEvidence) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            record.status == EvidenceStatus::Asserted
                && record.kind == EvidenceKind::Domain(expected)
        })
        .count()
}

fn asserted_unshadowed_global_count(il: &Il, name: &str) -> usize {
    let name_hash = stable_symbol_hash(name);
    il.evidence
        .iter()
        .filter(|record| {
            record.status == EvidenceStatus::Asserted
                && record.kind
                    == EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
        })
        .count()
}
