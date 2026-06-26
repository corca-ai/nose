use super::*;

#[test]
fn post_lowering_emits_ruby_string_affix_only_for_proven_receivers() {
    let interner = Interner::new();
    let prefix =
        nose_semantics::library_method_call_contract(Lang::Ruby, "start_with?", 1).unwrap();
    let suffix = nose_semantics::library_method_call_contract(Lang::Ruby, "end_with?", 1).unwrap();

    let literal_prefix = lower_fixture(
        "ruby_literal_prefix.rb",
        br#"def f
  "prelude".start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    let prefix_records = contract_api_records(&literal_prefix.evidence, prefix.id, prefix.callee);
    assert_eq!(prefix_records.len(), 1);
    assert!(
        api_record_depends_on_string_literal_domain(&literal_prefix.evidence, prefix_records[0]),
        "literal Ruby string receivers should carry explicit string-domain proof"
    );

    let literal_suffix = lower_fixture(
        "ruby_literal_suffix.rb",
        br#"def f
  "prelude".end_with?("ude")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&literal_suffix.evidence, suffix.id, suffix.callee),
        1
    );
    assert_eq!(
        contract_api_count(&literal_suffix.evidence, prefix.id, prefix.callee),
        0,
        "Ruby prefix and suffix contracts must stay direction-sensitive"
    );

    let custom_receiver = lower_fixture(
        "ruby_custom_receiver.rb",
        br#"class Token
  def start_with?(prefix)
    prefix == "pre"
  end
end

def f(token)
  token.start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&custom_receiver.evidence, prefix.id, prefix.callee),
        0,
        "custom Ruby same-name methods must not be admitted without string receiver proof"
    );

    let wrong_receiver = lower_fixture(
        "ruby_wrong_receiver.rb",
        br#"def f
  [:prelude].start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&wrong_receiver.evidence, prefix.id, prefix.callee),
        0,
        "non-string Ruby receivers must stay closed"
    );

    let multi_affix = lower_fixture(
        "ruby_multi_affix.rb",
        br#"def f
  "prelude".start_with?("x", "pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&multi_affix.evidence, prefix.id, prefix.callee),
        0,
        "Ruby multi-affix calls must stay closed until disjunction semantics exist"
    );

    let monkey_patch = lower_fixture(
        "ruby_monkey_patch.rb",
        br#"class String
  def start_with?(prefix)
    false
  end
end

def f
  "prelude".start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&monkey_patch.evidence, prefix.id, prefix.callee),
        0,
        "same-file Ruby String#start_with? redefinitions must close admission"
    );

    let class_eval_patch = lower_fixture(
        "ruby_class_eval_patch.rb",
        br#"String.class_eval do
  def start_with?(prefix)
    false
  end
end

def f
  "prelude".start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&class_eval_patch.evidence, prefix.id, prefix.callee),
        0,
        "same-file Ruby String.class_eval redefinitions must close admission"
    );

    let define_method_patch = lower_fixture(
        "ruby_define_method_patch.rb",
        br#"class String
  define_method(:start_with?) do |prefix|
    false
  end
end

def f
  "prelude".start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(&define_method_patch.evidence, prefix.id, prefix.callee),
        0,
        "same-file Ruby String#define_method redefinitions must close admission"
    );

    let class_eval_define_method_patch = lower_fixture(
        "ruby_class_eval_define_method_patch.rb",
        br#"String.class_eval do
  define_method(:start_with?) do |prefix|
    false
  end
end

def f
  "prelude".start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &class_eval_define_method_patch.evidence,
            prefix.id,
            prefix.callee
        ),
        0,
        "same-file Ruby String.class_eval define_method redefinitions must close admission"
    );

    let direct_define_method_patch = lower_fixture(
        "ruby_direct_define_method_patch.rb",
        br#"String.define_method(:start_with?) do |prefix|
  false
end

def f
  "prelude".start_with?("pre")
end
"#,
        Lang::Ruby,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &direct_define_method_patch.evidence,
            prefix.id,
            prefix.callee
        ),
        0,
        "same-file Ruby String.define_method redefinitions must close admission"
    );
}

fn api_record_depends_on_string_literal_domain(
    evidence: &[EvidenceRecord],
    api_record: &EvidenceRecord,
) -> bool {
    api_record.dependencies.iter().any(|id| {
        evidence.iter().any(|record| {
            record.id == *id
                && matches!(
                    record.anchor,
                    EvidenceAnchor::Node {
                        kind: NodeKind::Lit,
                        ..
                    }
                )
                && record.kind == EvidenceKind::Domain(DomainEvidence::String)
        })
    })
}
