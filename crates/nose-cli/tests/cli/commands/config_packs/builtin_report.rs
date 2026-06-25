use super::*;

#[path = "builtin_report/group_0.rs"]
mod group_0;
#[path = "builtin_report/group_1.rs"]
mod group_1;
#[path = "builtin_report/group_2.rs"]
mod group_2;
#[path = "builtin_report/group_3.rs"]
mod group_3;

#[test]
fn query_json_reports_builtin_semantic_packs() {
    let dir = make_project("semantic_pack_builtin_report");
    let json = query_json(&run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    assert_eq!(
        json["semantic_packs"]
            .as_array()
            .expect("semantic_packs should be an array")
            .len(),
        46
    );

    group_0::assert_group(&json);
    group_1::assert_group(&json);
    group_2::assert_group(&json);
    group_3::assert_group(&json);
    let _ = fs::remove_dir_all(&dir);
}
