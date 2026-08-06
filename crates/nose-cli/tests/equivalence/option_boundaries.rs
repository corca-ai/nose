use super::*;

#[test]
fn rust_constructor_pattern_variant_test_stays_distinct() {
    let i = Interner::new();
    // #390: a binding constructor pattern's variant test lowers to the constructor PATH (the
    // discriminant), not the whole pattern as an opaque Raw node. The discriminant must still
    // discriminate — matching `Some(_)` vs `Ok(_)` are different variants and stay distinct, even
    // now that binding extraction makes the *bodies* converge (see
    // `rust_constructor_pattern_binding_extraction_converges`): the arm conditions still differ.
    let some = "pub fn f(x: Option<i32>) -> i32 { match x { Some(a) => a + 1, None => 0 } }\n";
    let ok = "pub fn f(x: Result<i32, i32>) -> i32 { match x { Ok(a) => a + 1, Err(_) => 0 } }\n";
    assert_ne!(
        value_fp(&i, some, Lang::Rust),
        value_fp(&i, ok, Lang::Rust),
        "Some(_) and Ok(_) are different variants — must stay distinct"
    );
}

#[test]
fn rust_constructor_pattern_binding_extraction_converges() {
    // #390 follow-up: a match arm projects its payload binding (`Some(v)` → `v = x.0`) ahead of
    // the body so the body's uses of it alpha-canonicalize. Two copies that differ ONLY in the
    // bound name now converge — closing the split the #390 lowering left open.
    let i = Interner::new();
    let some_a =
        "pub fn f(x: Option<i32>) -> i32 { match x { Some(a) => a * 2 + 1, None => 0 } }\n";
    let some_b =
        "pub fn g(x: Option<i32>) -> i32 { match x { Some(b) => b * 2 + 1, None => 0 } }\n";
    assert_eq!(
        value_fp(&i, some_a, Lang::Rust),
        value_fp(&i, some_b, Lang::Rust),
        "`Some(a) => a*2+1` and `Some(b) => b*2+1` differ only in the bound name — must converge"
    );
    // The body still gates: a different arm computation must NOT merge (no false merge).
    let some_c =
        "pub fn h(x: Option<i32>) -> i32 { match x { Some(c) => c * 3 + 1, None => 0 } }\n";
    assert_ne!(
        value_fp(&i, some_a, Lang::Rust),
        value_fp(&i, some_c, Lang::Rust),
        "different arithmetic in the arm body must stay distinct"
    );
    // Cross-variant stays distinct even though both bodies are now `v = x.0; …` (the arm
    // *condition* — `x == Some` vs `x == Ok` — keeps them apart).
    let ok_a =
        "pub fn k(x: Result<i32, i32>) -> i32 { match x { Ok(a) => a * 2 + 1, Err(_) => 0 } }\n";
    assert_ne!(
        value_fp(&i, some_a, Lang::Rust),
        value_fp(&i, ok_a, Lang::Rust),
        "Some and Ok are different variants — must stay distinct after binding extraction"
    );
}

#[test]
fn option_defaulting_converges_with_nullish_default_boundaries() {
    let i = Interner::new();
    let js = "function f(value, fallback, other, otherDefault) { return value ?? fallback; }";
    let js_guard = "function f(value, fallback, other, otherDefault) { if (value == null) { return fallback; } return value; }";
    let ts_guard = "function f(value: number | null | undefined, fallback: number, other: number | null | undefined, otherDefault: number): number { return value == null ? fallback : value; }";
    let rust_unwrap = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.unwrap_or(fallback) }\n";
    let rust_unwrap_else = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.unwrap_or_else(|| fallback) }\n";
    let rust_map_or = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.map_or(fallback, |inner| inner) }\n";
    let rust_guard = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { if value.is_some() { value.unwrap_or(fallback) } else { fallback } }\n";
    let swift_coalesce = "func f(_ value: Int?, _ fallback: Int, _ other: Int?, _ otherDefault: Int) -> Int {\n    return value ?? fallback\n}\n";
    let wrong_default = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.unwrap_or(other_default) }\n";
    let wrong_value = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { other.unwrap_or(fallback) }\n";
    let swift_wrong_default = "func f(_ value: Int?, _ fallback: Int, _ other: Int?, _ otherDefault: Int) -> Int {\n    return value ?? otherDefault\n}\n";
    let swift_wrong_value = "func f(_ value: Int?, _ fallback: Int, _ other: Int?, _ otherDefault: Int) -> Int {\n    return other ?? fallback\n}\n";
    let swift_effectful_default = "func expensive() -> Int {\n    return 1\n}\n\nfunc f(_ value: Int?, _ fallback: Int, _ other: Int?) -> Int {\n    return value ?? expensive()\n}\n";
    let swift_computed_property_default = "struct Source {\n    var fallback: Int {\n        return 1\n    }\n}\n\nfunc f(_ value: Int?, _ source: Source, _ other: Int?) -> Int {\n    return value ?? source.fallback\n}\n";
    let swift_custom_coalesce = "struct Box {}\nfunc ??(lhs: Box, rhs: Int) -> Int {\n    return rhs\n}\n\nfunc f(_ value: Box, _ fallback: Int, _ other: Box) -> Int {\n    return value ?? fallback\n}\n";
    let swift_optional_coalesce_overload = "struct Box {}\nfunc ??(lhs: Box?, rhs: Int) -> Int {\n    return rhs + 1\n}\n\nfunc f(_ value: Box?, _ fallback: Int, _ other: Box?) -> Int {\n    return value ?? fallback\n}\n";
    let truthy_or =
        "function f(value, fallback, other, otherDefault) { return value || fallback; }";
    let shadowed_undefined = "function f(value, fallback, other, otherDefault, undefined) { return value === undefined ? fallback : value; }";

    let fp = value_fp(&i, js, Lang::JavaScript);
    assert_fingerprint_cases_converge(
        &i,
        &fp,
        [
            fp_case!(js_guard, JavaScript),
            fp_case!(ts_guard, TypeScript),
            fp_case!(rust_unwrap, Rust),
            fp_case!(rust_unwrap_else, Rust),
            fp_case!(rust_map_or, Rust),
            fp_case!(rust_guard, Rust),
            fp_case!(swift_coalesce, Swift),
        ],
    );
    assert_fingerprint_cases_stay_split(
        &i,
        &fp,
        [
            fp_case!(wrong_default, Rust),
            fp_case!(wrong_value, Rust),
            fp_case!(swift_wrong_default, Swift),
            fp_case!(swift_wrong_value, Swift),
            named_fp_case!(swift_effectful_default, Swift, "f"),
            named_fp_case!(swift_computed_property_default, Swift, "f"),
            named_fp_case!(swift_custom_coalesce, Swift, "f"),
            named_fp_case!(swift_optional_coalesce_overload, Swift, "f"),
            fp_case!(truthy_or, JavaScript),
            fp_case!(shadowed_undefined, JavaScript),
        ],
    );
}

const RUBY_NIL_PREDICATE_ISEQ_BOUNDARIES: &[(&str, &str)] = &[
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence eval string define_method"),
    ("iseq = RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\")\niseq.eval\n\ndef f(value, other)\n  value.nil?\nend\n", "stored InstructionSequence eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").send(:eval)\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence send eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").send(*[:eval])\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence splat send eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").itself.eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence itself eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").then { |iseq| iseq }.eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence then eval string define_method"),
    ("[RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\")].first.eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence array first eval string define_method"),
    ("bin = RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").to_binary\nRubyVM::InstructionSequence.load_from_binary(bin).eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence load_from_binary eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").method(:eval).call\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence Method#call eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").public_method(:eval).call\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence public Method#call eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").send(:method, :eval).call\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence send-acquired Method#call eval string define_method"),
    ("RubyVM.const_get(:InstructionSequence).compile(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence const_get eval string define_method"),
    ("RubyVM.public_send(:const_get, :InstructionSequence).compile(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence public_send const_get eval string define_method"),
    ("Object.const_get(\"RubyVM::InstructionSequence\").compile(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence Object const_get full path eval string define_method"),
    ("Object.const_get(:RubyVM).const_get(:InstructionSequence).compile(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence nested Object const_get eval string define_method"),
    ("RubyVM::InstructionSequence.method(:compile).call(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence Method#call compile then eval string define_method"),
    ("factory = RubyVM::InstructionSequence.method(:compile)\nfactory.call(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "stored InstructionSequence Method#call compile then eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").then { |iseq| iseq.eval }\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence then block eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").instance_exec { eval }\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence instance_exec block eval string define_method"),
    ("RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\").then(&:eval)\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence Symbol#to_proc eval string define_method"),
    ("[RubyVM::InstructionSequence.compile(\"define_method(:nil?) { true }\")].each { |iseq| iseq.eval }\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence array each block eval string define_method"),
    ("RubyVM::InstructionSequence.method(:compile)[\"define_method(:nil?) { true }\"].eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence Method#[] compile then eval string define_method"),
    ("RubyVM::InstructionSequence.singleton_class.instance_method(:compile).bind(RubyVM::InstructionSequence).call(\"define_method(:nil?) { true }\").eval\n\ndef f(value, other)\n  value.nil?\nend\n", "InstructionSequence UnboundMethod#bind compile then eval string define_method"),
];

#[test]
fn ruby_nil_predicate_converges_with_null_absence_and_preserves_boundaries() {
    let i = Interner::new();
    let py_missing = "def f(value, other):\n    return value is None\n";
    let ruby_missing = "def f(value, other)\n  value.nil?\nend\n";
    let ruby_present = "def f(value, other)\n  !value.nil?\nend\n";
    let ruby_wrong_value = "def f(value, other)\n  other.nil?\nend\n";
    let ruby_rebound = "def f(value, other)\n  value = other\n  value.nil?\nend\n";
    let ruby_api_mutation_boundaries = [
        (
            "class Box\n  def nil?\n    true\n  end\nend\n\ndef f(value, other)\n  value.nil?\nend\n",
            "class method",
        ),
        ("def value.nil?\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "singleton method"),
        ("def nil?\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "top-level method"),
        ("class Object\n  alias nil? object_id\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "alias"),
        ("class Object\n  undef_method :nil?\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "undef"),
        ("define_method(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "define_method"),
        ("def f(value, other)\n  value.define_singleton_method(:nil?) { true }\n  value.nil?\nend\n", "define_singleton_method"),
        ("name = :nil?\ndefine_method(name) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic define_method"),
        ("def f(value, other)\n  name = :nil?\n  value.define_singleton_method(name) { true }\n  value.nil?\nend\n", "dynamic define_singleton_method"),
        ("class Object\n  name = :nil?\n  alias_method name, :object_id\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic alias_method"),
        ("send(:define_method, :nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "send define_method"),
        ("def f(value, other)\n  value.public_send(:define_singleton_method, :nil?) { true }\n  value.nil?\nend\n", "public_send define_singleton_method"),
        ("mutator = :define_method\nsend(mutator, :nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic send define_method"),
        ("method(:define_method).call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#call define_method"),
        ("def f(value, other)\n  value.method(:define_singleton_method).call(:nil?) { true }\n  value.nil?\nend\n", "receiver Method#call define_singleton_method"),
        ("mutator = :define_method\nmethod(mutator).call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic Method#call define_method"),
        ("m = method(:define_method)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "stored Method#call define_method"),
        ("def f(value, other)\n  m = value.method(:define_singleton_method)\n  m.call(:nil?) { true }\n  value.nil?\nend\n", "stored receiver Method#call define_singleton_method"),
        ("mutator = :define_method\nm = method(mutator)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "stored dynamic Method#call define_method"),
        ("m = method(:define_method)\nm[:nil?] do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "stored Method#[] define_method"),
        ("m = method(:define_method)\nm.(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "stored Method#call shorthand define_method"),
        ("p = method(:define_method).to_proc\np.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "stored Method#to_proc.call define_method"),
        ("m = method(:define_method)\np = m.to_proc\np.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "aliased Method#to_proc.call define_method"),
        ("m = method(:define_method)\nm.[](:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "explicit Method#[] define_method"),
        ("m = method(:define_method)\nm.===(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "explicit Method#=== define_method"),
        ("m = method(:define_method)\nm.send(:call, :nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "reflective Method#call define_method"),
        ("p = method(:define_method).to_proc\np.yield(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#to_proc Proc#yield define_method"),
        ("p = method(:define_method).curry\np.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#curry define_method"),
        ("p = method(:define_method).to_proc.curry\np.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#to_proc.curry define_method"),
        ("m = method(:define_method).unbind.bind(self)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#unbind.bind define_method"),
        ("m = method(:define_method).clone\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#clone define_method"),
        ("m = method(:define_method).itself\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#itself define_method"),
        ("m = method(:define_method).tap {}\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#tap define_method"),
        ("m = method(:define_method).then { |method_object| method_object }\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "nested Method object wrapper define_method"),
        ("m = method(:define_method)\nx = [m].first\nx.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "array-transferred Method object define_method"),
        ("m = method(:define_method)\nx = {k: m}[:k]\nx.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "hash-transferred Method object define_method"),
        ("method(:define_method).tap do |m|\n  m.call(:nil?) { true }\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "tap block Method object define_method"),
        ("method(:define_method).then do |m|\n  m.call(:nil?) { true }\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "then block Method object define_method"),
        ("m = send(:method, :define_method)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "send-acquired Method object define_method"),
        ("m = public_send(:method, :define_method)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "public_send-acquired Method object define_method"),
        ("m = Module.send(:instance_method, :define_method).bind(Object)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "send-acquired UnboundMethod define_method"),
        ("m = method(:method).call(:define_method)\nm.call(:nil?) do\n  true\nend\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#call-acquired Method object define_method"),
        ("eval(\"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "eval string define_method"),
        ("Object.module_eval(\"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "module_eval string define_method"),
        ("Object.class_eval(\"alias_method :nil?, :object_id\")\n\ndef f(value, other)\n  value.nil?\nend\n", "class_eval string alias_method"),
        ("send(:eval, \"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "send eval string define_method"),
        ("Object.send(:module_eval, \"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "send module_eval string define_method"),
        ("Object.public_send(:class_eval, \"alias_method :nil?, :object_id\")\n\ndef f(value, other)\n  value.nil?\nend\n", "public_send class_eval string alias_method"),
        ("Object.__send__(:instance_eval, \"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "__send__ instance_eval string define_method"),
        ("mutator = :eval\nsend(mutator, \"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic send eval string define_method"),
        ("m = method(:eval)\nm.call(\"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#call eval string define_method"),
        ("m = Object.method(:module_eval)\nm.call(\"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "Method#call module_eval string define_method"),
        ("mutator = :eval\nm = method(mutator)\nm.call(\"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic Method#call eval string define_method"),
        ("send(*[:eval, \"define_method(:nil?) { true }\"])\n\ndef f(value, other)\n  value.nil?\nend\n", "splat send eval string define_method"),
        ("Object.send(*[:module_eval, \"define_method(:nil?) { true }\"])\n\ndef f(value, other)\n  value.nil?\nend\n", "splat send module_eval string define_method"),
        ("Object.public_send(*[:class_eval, \"alias_method :nil?, :object_id\"])\n\ndef f(value, other)\n  value.nil?\nend\n", "splat public_send class_eval string alias_method"),
        ("args = [:eval, \"define_method(:nil?) { true }\"]\nsend(*args)\n\ndef f(value, other)\n  value.nil?\nend\n", "dynamic splat send eval string define_method"),
        ("m = method(:eval)\nm.send(*[:call, \"define_method(:nil?) { true }\"])\n\ndef f(value, other)\n  value.nil?\nend\n", "splat Method#send call eval string define_method"),
        ("m = send(*[:method, :eval])\nm.call(\"define_method(:nil?) { true }\")\n\ndef f(value, other)\n  value.nil?\nend\n", "splat send-acquired Method object eval string define_method"),
    ];
    let fp = value_fp(&i, py_missing, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_missing, Lang::Ruby));
    assert_ne!(fp, value_fp(&i, ruby_present, Lang::Ruby));
    assert_ne!(fp, value_fp(&i, ruby_wrong_value, Lang::Ruby));
    for (source, label) in ruby_api_mutation_boundaries {
        assert_ne!(
            fp,
            value_fp_named(&i, source, Lang::Ruby, "f"),
            "Ruby nil? API mutation must stay split: {label}"
        );
    }
    for (source, label) in RUBY_NIL_PREDICATE_ISEQ_BOUNDARIES {
        assert_ne!(
            fp,
            value_fp_named(&i, source, Lang::Ruby, "f"),
            "Ruby nil? ISeq mutation must stay split: {label}"
        );
    }
    assert_ne!(fp, value_fp(&i, ruby_rebound, Lang::Ruby));
}

#[test]
fn swift_optional_nil_presence_requires_optional_coordinate() {
    let i = Interner::new();
    let py_missing = "def f(value, other):\n    return value is None\n";
    let py_present = "def f(value, other):\n    return value is not None\n";
    let swift_missing =
        "func f(_ value: Int?, _ other: Int?) -> Bool {\n    return value == nil\n}\n";
    let swift_missing_reversed =
        "func f(_ value: Int?, _ other: Int?) -> Bool {\n    return nil == value\n}\n";
    let swift_present =
        "func f(_ value: Int?, _ other: Int?) -> Bool {\n    return value != nil\n}\n";
    let swift_wrong_value =
        "func f(_ value: Int?, _ other: Int?) -> Bool {\n    return other == nil\n}\n";
    let swift_rebound = "func f(_ original: Int?, _ other: Int?) -> Bool {\n    var value = original\n    value = other\n    return value == nil\n}\n";
    let swift_custom_nil = "struct Box: ExpressibleByNilLiteral {\n    init(nilLiteral: ()) {}\n}\n\nfunc ==(lhs: Box, rhs: Box) -> Bool {\n    return false\n}\n\nfunc f(_ value: Box, _ other: Box) -> Bool {\n    return value == nil\n}\n";
    let swift_optional_equality_overload = "struct Box {}\nfunc ==(lhs: Box?, rhs: Box?) -> Bool {\n    return false\n}\n\nfunc f(_ value: Box?, _ other: Box?) -> Bool {\n    return value == nil\n}\n";

    let missing_fp = value_fp(&i, py_missing, Lang::Python);
    let present_fp = value_fp(&i, py_present, Lang::Python);
    assert_fingerprint_cases_converge(
        &i,
        &missing_fp,
        [
            fp_case!(swift_missing, Swift),
            fp_case!(swift_missing_reversed, Swift),
        ],
    );
    assert_fingerprint_cases_converge(&i, &present_fp, [fp_case!(swift_present, Swift)]);
    assert_fingerprint_cases_stay_split(
        &i,
        &missing_fp,
        [
            fp_case!(swift_present, Swift),
            fp_case!(swift_wrong_value, Swift),
            fp_case!(swift_rebound, Swift),
            named_fp_case!(swift_custom_nil, Swift, "f"),
            named_fp_case!(swift_optional_equality_overload, Swift, "f"),
        ],
    );
}

#[test]
fn repeated_nullish_default_with_same_fallback_collapses() {
    let i = Interner::new();
    let single = "function f(value, fallback, otherDefault) { return value ?? fallback; }";
    let repeated =
        "function f(value, fallback, otherDefault) { return (value ?? fallback) ?? fallback; }";
    let different_default =
        "function f(value, fallback, otherDefault) { return (value ?? fallback) ?? otherDefault; }";
    let fp = value_fp(&i, single, Lang::JavaScript);
    assert_eq!(fp, value_fp(&i, repeated, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, different_default, Lang::JavaScript));
}

#[test]
fn rust_if_let_option_presence_converges_with_option_predicates() {
    let i = Interner::new();
    let if_some = "pub fn f(value: Option<i32>) -> bool {\n    if let Some(_) = value { true } else { false }\n}\n";
    let is_some = "pub fn g(value: Option<i32>) -> bool {\n    value.is_some()\n}\n";
    let if_none = "pub fn h(value: Option<i32>) -> bool {\n    if let None = value { true } else { false }\n}\n";
    let shadowed_some_pattern = "struct Some<T>(T);\npub fn f(value: Some<i32>) -> bool {\n    if let Some(_) = value { true } else { false }\n}\n";
    assert_eq!(
        value_fp(&i, if_some, Lang::Rust),
        value_fp(&i, is_some, Lang::Rust),
        "if let Some(_) should converge with is_some()"
    );
    assert_ne!(
        value_fp(&i, if_some, Lang::Rust),
        value_fp(&i, if_none, Lang::Rust),
        "if let Some(_) must stay distinct from if let None"
    );
    assert_ne!(
        value_fp(&i, if_some, Lang::Rust),
        value_fp_named(&i, shadowed_some_pattern, Lang::Rust, "f"),
        "a local Rust Some pattern must not be treated as Option::Some"
    );
}

#[test]
fn rust_if_let_result_channels_converge_with_result_predicates() {
    let i = Interner::new();
    let if_ok = "pub fn f(value: Result<i32, i32>) -> bool {\n    if let Ok(_) = value { true } else { false }\n}\n";
    let is_ok = "pub fn g(value: Result<i32, i32>) -> bool {\n    value.is_ok()\n}\n";
    let if_err = "pub fn h(value: Result<i32, i32>) -> bool {\n    if let Err(_) = value { true } else { false }\n}\n";
    let is_err = "pub fn i(value: Result<i32, i32>) -> bool {\n    value.is_err()\n}\n";
    let shadowed_ok = "struct Ok<T>(T);\npub fn f(value: Ok<i32>) -> bool {\n    if let Ok(_) = value { true } else { false }\n}\n";
    let shadowed_result_is_ok = "struct Result<T, E> { value: T, err: E }\nimpl<T, E> Result<T, E> { fn is_ok(&self) -> bool { false } }\npub fn f(value: Result<i32, i32>) -> bool {\n    value.is_ok()\n}\n";
    let result_unwrap_else = "pub fn f(value: Result<i32, i32>, fallback: i32) -> i32 {\n    value.unwrap_or_else(|_| fallback)\n}\n";
    let result_fallback =
        "pub fn g(value: Result<i32, i32>, fallback: i32) -> i32 {\n    fallback\n}\n";

    assert_eq!(
        value_fp(&i, if_ok, Lang::Rust),
        value_fp(&i, is_ok, Lang::Rust),
        "if let Ok(_) should converge with is_ok()"
    );
    assert_eq!(
        value_fp(&i, if_err, Lang::Rust),
        value_fp(&i, is_err, Lang::Rust),
        "if let Err(_) should converge with is_err()"
    );
    assert_ne!(
        value_fp(&i, if_ok, Lang::Rust),
        value_fp(&i, if_err, Lang::Rust),
        "Ok and Err channels must stay distinct"
    );
    assert_ne!(
        value_fp(&i, if_ok, Lang::Rust),
        value_fp_named(&i, shadowed_ok, Lang::Rust, "f"),
        "a local Rust Ok pattern must not be treated as Result::Ok"
    );
    assert_ne!(
        value_fp(&i, is_ok, Lang::Rust),
        value_fp_named(&i, shadowed_result_is_ok, Lang::Rust, "f"),
        "a local Rust Result receiver must not be treated as std Result::is_ok"
    );
    assert_ne!(
        value_fp(&i, result_unwrap_else, Lang::Rust),
        value_fp(&i, result_fallback, Lang::Rust),
        "Result callback/defaulting APIs are not admitted by the narrow predicate slice"
    );
}
