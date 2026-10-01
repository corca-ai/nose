use super::{navigation::Navigation, reviews, selection::Observations};
use nose_detect::regions::evolution::Change;
use serde_json::{json, Value};

pub(super) fn describe(
    rows: &[&Change],
    index: &Observations<'_>,
    navigation: &Navigation<'_>,
) -> Value {
    if rows.len() != 1 || rows[0].after.len() != 1 {
        return json!({"available":false,"reason":"Review recording requires exactly one current family; ambiguous or unresolved relations cannot choose a target."});
    }
    let row = rows[0];
    if !reviews::source_complete(index.after[&row.after[0]]) {
        return json!({"available":false,"reason":"The current family lacks a review key or complete captured source evidence."});
    }
    json!({
        "available":true,
        "command_prefix":navigation.source_json(vec![format!("change={}", row.id.hex())]),
        "required_arguments":{
            "--write-review":"New file path; never overwrites an existing file",
            "--decision":["keep-separate","refactor","defer"],
            "--reason":"Nonempty caller explanation of this current family decision"
        },
        "meaning":"Append caller-supplied arguments to record a capture-bound decision. No default decision, finding suppression or source edit is implied."
    })
}
