use crate::{unit_dags_at, DetectOptions};
use nose_il::{FileId, Interner, Lang};

#[test]
fn witness_locations_follow_unit_occurrences_not_file_wide_interning() {
    let interner = Interner::new();
    let source = b"function file() {\n  return { projectId: \"project_1\", content: \"\", count: 1 };\n}\nconst detail = { id: \"project_1\" };\n";
    let il = nose_frontend::lower_source(FileId(0), "a.ts", source, Lang::TypeScript, &interner)
        .unwrap();
    let dag = unit_dags_at(&il, &interner, &DetectOptions::default(), &[(1, 3)])
        .remove(0)
        .unwrap()
        .0;
    assert!(
        dag.nodes.iter().all(|n| n.line_start <= 3),
        "outside-unit creation spans are not occurrence evidence"
    );
    assert!(dag
        .nodes
        .iter()
        .any(|n| n.op == nose_normalize::VgOp::Const && n.line_start == 2));
}
