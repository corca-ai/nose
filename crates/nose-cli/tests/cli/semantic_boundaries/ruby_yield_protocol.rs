use super::*;

/// Ruby `yield a, b` invokes the current method block and may run arbitrary
/// callback effects. It must not collapse into Ruby's ordinary multiple-value
/// return shape until a block-yield protocol contract exists.
#[test]
fn query_mode_semantic_rejects_unproven_ruby_yield_callback_convergence() {
    let project = TempProject::new("ruby_yield_protocol_boundary");
    project.write(
        "return_pair.rb",
        "def produce(a, b, &block)\n  return a, b\nend\n",
    );
    project.write(
        "yield_pair.rb",
        "def produce(a, b, &block)\n  yield a, b\nend\n",
    );
    project.write(
        "block_call_pair.rb",
        "def produce(a, b, &block)\n  block.call(a, b)\nend\n",
    );

    let json = project.query_json("semantic", &["--min-size", "1", "--min-lines", "1"]);
    for pair in [
        ["return_pair.rb", "yield_pair.rb"],
        ["block_call_pair.rb", "yield_pair.rb"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "Ruby yield must not be erased into ordinary return or direct block call without callback demand/effect proof for {pair:?}: {json}"
        );
    }
}
