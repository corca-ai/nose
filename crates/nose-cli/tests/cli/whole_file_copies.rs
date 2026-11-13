use super::*;

#[test]
fn byte_identical_small_units_form_a_family_issue_1009() {
    let project = TempProject::new("whole_file_copy");
    let source: String = (0..40)
        .map(|n| {
            format!("export function step{n}(value: number): number {{ return value + {n}; }}\n")
        })
        .collect();
    let source = format!("{source}export const version = 1;\n");
    project.write("a.ts", &source);
    project.write("b.ts", &source);
    let cache = make_temp_dir("whole_file_copy_cache");
    for mode in ["semantic", "near", "semantic,near"] {
        let mut results = Vec::new();
        for cached in [false, true, true] {
            let mut command = Command::new(bin());
            command.args([
                "query",
                project.path().to_str().unwrap(),
                "--mode",
                mode,
                "--format",
                "json",
            ]);
            if cached {
                command.args(["--cache-dir", cache.to_str().unwrap()]);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                !result["families"].as_array().unwrap().is_empty(),
                "{mode}: {result}"
            );
            let family = &result["families"][0];
            assert_eq!(family["witness"], "copy-paste");
            assert_eq!(family["members"], 2);
            for location in family["locations"].as_array().unwrap() {
                assert_eq!(location["start"], 1);
                assert_eq!(location["end"], 41);
                assert_eq!(
                    location["region"]["content_digest"],
                    location["region"]["source_digest"]
                );
            }
            results.push(result);
        }
        assert_same_analysis(&results[0], &results[1]);
        assert_same_analysis(&results[0], &results[2]);
    }
    // Equal raw IL is weaker than equal source: a comment-only change must not
    // sneak through the narrow whole-source route.
    project.write("b.ts", &format!("// changed source\n{source}"));
    let changed = project.query_json("semantic,near", &[]);
    assert!(query_families(&changed)
        .iter()
        .all(|family| family["witness"] != "copy-paste"));
    // Suppression cannot be bypassed by treating the enclosing file as a copy.
    let suppressed = format!("// nose-ignore\n{source}");
    project.write("a.ts", &suppressed);
    project.write("b.ts", &suppressed);
    let ignored = project.query_json("semantic,near", &[]);
    assert!(query_families(&ignored)
        .iter()
        .all(|family| family["witness"] != "copy-paste"));
    fs::remove_dir_all(cache).unwrap();
}
