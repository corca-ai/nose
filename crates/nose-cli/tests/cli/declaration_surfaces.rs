use super::*;

fn default_query_json(path: &str, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["query", path];
    args.extend_from_slice(extra);
    args.extend(["--format", "json"]);
    query_json(&run_raw(&args))
}

#[test]
fn query_declaration_only_type_contract_is_omitted_reasoned_and_recoverable() {
    let project = TempProject::new("declaration_only_type_contract");
    project.write(
        "contracts.ts",
        "interface FirstFactory {\n  new (\n    a: number,\n    b: number,\n    c: number,\n    d: number,\n    e: number,\n    f: number,\n    g: number,\n    h: number\n  ): Product;\n}\n\ninterface SecondFactory {\n  new <A, B, C>(\n    a: number,\n    b: number,\n    c: number,\n    d: number,\n    e: number,\n    f: number,\n    g: number,\n    h: number\n  ): Product;\n}\n\ntype ThirdFactory = {\n  new (\n    a: number,\n    b: number,\n    c: number,\n    d: number,\n    e: number,\n    f: number,\n    g: number,\n    h: number\n  ): Product;\n};\n\ninterface FourthFactory {\n  new <VeryLongTypeParameter, AnotherLongTypeParameter>(\n    a: number,\n    b: number,\n    c: number,\n    d: number,\n    e: number,\n    f: number,\n    g: number,\n    h: number\n  ): Product;\n}\n",
    );
    let path = project.path().to_str().unwrap();
    let all = default_query_json(
        path,
        &["all", "top=0", "--min-size", "1", "--min-lines", "4"],
    );
    let family = query_families(&all)
        .iter()
        .find(|row| {
            row["surface"] == "declaration"
                && row["members"].as_u64().is_some_and(|members| members >= 2)
        })
        .unwrap_or_else(|| panic!("full JSON must retain the declaration-only type family: {all}"));
    assert_eq!(family["surface"], "declaration");
    let family_id = family["id"].as_str().unwrap();

    let default = default_query_json(path, &[]);
    assert!(
        query_families(&default)
            .iter()
            .all(|row| row["id"] != family_id),
        "the bare default must omit the non-actionable contract: {default}"
    );

    let filtered = default_query_json(
        path,
        &[
            "surface=declaration",
            "top=0",
            "--min-size",
            "1",
            "--min-lines",
            "4",
        ],
    );
    assert!(
        query_families(&filtered)
            .iter()
            .any(|row| row["id"] == family_id),
        "the declaration filter must recover the family: {filtered}"
    );
    for term in ["surface=default", "surface~default", "surface!=declaration"] {
        let excluded = default_query_json(
            path,
            &[term, "top=0", "--min-size", "1", "--min-lines", "4"],
        );
        assert!(
            query_families(&excluded)
                .iter()
                .all(|row| row["id"] != family_id),
            "{term} must exclude the declaration family: {excluded}"
        );
    }

    let id_term = format!("id={family_id}");
    let opened = run(&[
        "query",
        path,
        &id_term,
        "--format",
        "json",
        "--min-size",
        "1",
        "--min-lines",
        "4",
    ]);
    let opened = query_json(&opened);
    assert_eq!(opened["family"]["id"], family_id);
    assert_eq!(opened["family"]["surface"], "declaration");

    let default_human = run(&["query", path]);
    assert!(
        default_human.contains("declaration-only-type-contract")
            && !default_human.contains("contracts.ts"),
        "human output must explain the omission without listing it: {default_human}"
    );

    let gated = run(&["query", path, "--fail-on", "any"]);
    assert!(
        gated.contains("declaration-only-type-contract"),
        "an omitted contract must not trip the default gate: {gated}"
    );

    let sarif = run_raw(&["query", path, "--format", "sarif"]);
    assert!(
        !sarif.contains(family_id),
        "default SARIF must omit the non-actionable declaration family: {sarif}"
    );
}
