/// Cache/watch settings travel with `next`; work limits do not alter successful findings.
/// Compare the complete
/// analysis payload separately; exploration_ux executes the navigation contract.
pub(crate) fn assert_same_analysis(left: &serde_json::Value, right: &serde_json::Value) {
    fn analysis(mut value: serde_json::Value) -> serde_json::Value {
        match &mut value {
            serde_json::Value::Object(fields) => {
                fields.remove("next");
                if let Some(serde_json::Value::Object(context)) = fields.get_mut("analysis") {
                    context.remove("max_candidate_pairs");
                }
                for child in fields.values_mut() {
                    *child = analysis(child.take());
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    *child = analysis(child.take());
                }
            }
            _ => {}
        }
        value
    }
    assert_eq!(analysis(left.clone()), analysis(right.clone()));
}
