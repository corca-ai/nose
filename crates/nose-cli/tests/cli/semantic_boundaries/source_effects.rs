use super::*;

const LOOP: &str = "def f(xs, baseline, scale, seen):\n    total = baseline\n    squared = 0\n    count = 0\n    for x in xs:\n        total += x * scale + 7\n        squared += x * x\n        count += 1\n    return total * 3 + squared * 5 - count * 2 + baseline * 11 + scale * 13\n";

#[test]
fn default_native_query_keeps_erased_effect_twins_apart() {
    let result = "    return total";
    for (label, left, right) in [
        (
            "continue",
            LOOP.replace(
                result,
                "        if seen.append(x):\n            continue\n    return total",
            ),
            LOOP.to_owned(),
        ),
        (
            "empty_guard",
            LOOP.replace(
                result,
                "    if seen.append(1):\n        pass\n    return total",
            ),
            LOOP.to_owned(),
        ),
        (
            "for_else",
            LOOP.replace(result, "    else:\n        total += 100\n    return total"),
            LOOP.to_owned(),
        ),
        (
            "while_else",
            LOOP.replace("for x in xs:", "while count < scale:\n        x = count")
                .replace(result, "    else:\n        total += 100\n    return total"),
            LOOP.replace("for x in xs:", "while count < scale:\n        x = count"),
        ),
        (
            "delete",
            LOOP.replace(result, "    del seen[0]\n    return total"),
            LOOP.to_owned(),
        ),
        (
            "with",
            LOOP.replace(result, "    with seen:\n        return total"),
            LOOP.to_owned(),
        ),
        (
            "assert",
            LOOP.replace(result, "    assert scale\n    return total"),
            LOOP.to_owned(),
        ),
    ] {
        let project = TempProject::new(label);
        project.write("a.py", &left);
        project.write("b.py", &right);
        let json = project.query_semantic_json();
        assert!(
            query_families(&json)
                .iter()
                .all(|family| family["witness"] != "exact"
                    || !family["locations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|location| location["kind"] == "Function"
                            && location["is_fragment"] != true)),
            "{label}: {json}"
        );
    }
}

#[test]
fn default_native_query_keeps_quoted_content_apart() {
    let project = TempProject::new("string_content");
    let body = "def f(name, team, email, region, role):\n    return (\n        \"'name:'\" + name\n        + \"; team:\" + team\n        + \"; email:\" + email\n        + \"; region:\" + region\n        + \"; role:\" + role\n        + \"'end:'\"\n    )\n";
    project.write("a.py", body);
    project.write(
        "b.py",
        &body.replace("'name:'", "name:").replace("'end:'", "end:"),
    );
    assert!(query_families(&project.query_semantic_json()).is_empty());
}
