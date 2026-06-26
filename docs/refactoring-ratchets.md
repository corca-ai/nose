# Refactoring ratchets

nose keeps code quality pressure as ratchets: existing debt can be carried
temporarily, but it must not grow, and any real improvement should lower the
accepted ceiling in the same change.

The repository already ratchets function complexity and length through
[`clippy.toml`](../clippy.toml), test coverage through `cargo llvm-cov`, and
self-duplication through [`scripts/check-duplication.sh`](../scripts/check-duplication.sh).
The Rust file-length ratchet adds a coarser module-design signal on top.

## Rust file length

Run the gate directly with:

```sh
python3 scripts/check-file-lengths.py
```

The target is below 600 lines for every Rust file under `crates/`; the enforced
default max is 599. Files already above that line are recorded in
[`scripts/file-length-budgets.json`](../scripts/file-length-budgets.json). A
budgeted file fails the gate if it grows. It also fails if it shrinks without its
budget being lowered, so the accepted ceiling moves down whenever a refactor pays
down debt. The budget map should stay empty once all Rust files are below the
target.

CI runs the gate against the base ref with `--ratchet-base`, so the budget
file itself cannot be loosened in the same change: `default_max_lines` may not
increase, existing file budgets may not increase, and new over-target budget
entries are rejected.

Do not use the budget file to bless newly large modules. New modules should stay
under the 600-line target; if a split still produces a 600-line-or-larger file,
keep looking for a sharper boundary.

## CLI legacy prelude

Run the temporary prelude gate directly with:

```sh
python3 scripts/check-legacy-prelude.py
```

`nose-cli/src/legacy_prelude.rs` exists only to keep older CLI modules compiling
while they move to explicit owner imports. The gate counts top-level
`nose-cli/src/*.rs` modules that still import from `crate::legacy_prelude` and
the number of `pub(crate) use` exports inside the prelude itself. The current
budget is 17 users and 34 exports; future refactors should lower those budgets
when they remove users or exports, and new users should import from the owning
`crate::<module>` instead.

## Refactoring direction

File length is a symptom, not the objective. Prefer changes that make ownership
and behavior easier to reason about:

- separate CLI orchestration from query planning, rendering, config parsing, and
  file/process effects;
- keep the CLI binary root focused on process setup and subcommand dispatch;
  argument models, legacy detect/IL adapters, scan baseline handling, graded
  witness enrichment, opportunity grouping, source-line diff/proposal logic,
  human/markdown/SARIF scan rendering, and CLI-side timing helpers now live in
  dedicated `nose-cli/src/{cli_args,detect_command,il_command,scan_*,timing}.rs`
  modules;
- keep the CLI binary root as the process entry point only; command dispatch,
  scan/review detection setup, path diagnostics, terminal styling, runtime
  setup, shared report text, and CLI-root tests now live in
  `nose-cli/src/{command_dispatch,detect_pipeline,path_utils,style,runtime,report_text,main_tests/*}.rs`;
- keep the `nose-cli` crate root free of ambient helper imports. Remaining
  compatibility imports are isolated in `nose-cli/src/legacy_prelude.rs`; do not
  add new module dependencies there when a module can name its actual owner with
  `crate::<module>::...`, and shrink the prelude as files move to explicit imports;
- keep divergent-edit review split by adapter boundary; review detection policy,
  git/worktree diff plumbing, output formats, and tests now live under
  `nose-cli/src/review/`;
- keep `nose-il` roots as API indexes; units/domains/evidence facets, the arena
  and lazy indexes, the builder/corpus wrappers, node core/domain/evidence/source
  facts, and node operators now live in focused `nose-il/src/{unit*,il,builder,corpus,node/*}.rs`
  modules;
- keep shared verify-oracle support outside the command dispatcher; the oracle
  battery, behavior hashing, and behavioral-gate tally now live in
  `nose-cli/src/oracle_gate.rs`;
- keep `nose verify` collection separate from presentation; JSON, exclusion,
  soundness, completeness, calibration, and falsification output now live in
  `nose-cli/src/verify_report.rs`;
- keep the verify oracle's per-file collection path separate from command
  parsing; interpreted records, exclusion accounting, and canon-preservation
  collection now live in `nose-cli/src/verify_collect.rs`;
- keep hidden diagnostic and benchmark commands outside the dispatcher; `features`,
  `value-census`, `stats`, `eval`, and `ceiling` now live in
  `nose-cli/src/diagnostic_commands.rs`;
- keep the `nose query` surface outside the dispatcher: query model/JSON helpers,
  renderers, dashboard, family drilldown, and command orchestration now live in
  `nose-cli/src/query_*.rs`;
- keep shared scan dataset construction and deprecated `nose scan` command
  orchestration outside the dispatcher; they now live in
  `nose-cli/src/scan_commands.rs`;
- keep shared scan option parsing and report model types outside the dispatcher;
  mode parsing, scan-channel resolution, report formats, ranking keys, gate
  selectors, and scan scope summaries now live in `nose-cli/src/scan_options.rs`;
- keep deprecated `nose scan` report rendering and gate output outside the
  dispatcher; the JSON/markdown/human/SARIF format switch, hotspots, and
  reinvented-helper report section now live in `nose-cli/src/scan_report.rs`;
- keep post-lower Library API recognition out of the shared lowering context;
  the first pass lives in `nose-frontend/src/lower/library_api_post_lower.rs`;
- keep shared frontend control-flow lowering out of the shared lowering context;
  `switch`, `if`, `while`, block-wrapping, and C-style `for` helpers now live in
  `nose-frontend/src/lower/control_flow.rs`;
- keep the shared frontend lowering context as the small state/dispatch root;
  IL builders, semantic-evidence recording, import facts, parse/file setup,
  post-lower evidence helpers, expression helpers, and lowering tests now live
  in focused `nose-frontend/src/lower/*` modules;
- keep wide frontend language roots as dispatch surfaces instead of mixed
  parser-policy files; JS/TS import parsing, type declarations, declarations,
  control-flow rewrites, expression lowering, global-symbol proof, record-shape
  guards, JSX lowering, operators, syntax helpers, and tests now live in
  focused `nose-frontend/src/js_ts/*` modules;
- keep corpus-level import replacement split by concern; export discovery,
  binding-use safety, module-path hashing, snapshot/evidence copying, and tests
  now live in focused `nose-frontend/src/module_imports/*` modules;
- keep the verify oracle's value model separate from tree-walking evaluation;
  `Value`, `Behavior`, symbolic containment, and declared-domain coercion now
  live in `nose-normalize/src/interp/value.rs`;
- keep the verify oracle's primitive operation semantics separate from tree-walking
  evaluation; truthiness, builtin folds, ranges, int32 coercion, and unary/binary
  operator execution now live in `nose-normalize/src/interp/ops.rs`;
- keep the verify oracle root focused on entry points and execution state;
  field-state proof, statement execution, expression evaluation, call/builtin
  handling, higher-order evaluation, and oracle tests now live in focused
  `nose-normalize/src/interp/*` modules;
- keep builtin language-core call-target evidence as a small pass root; direct
  in-file function targets, scope/binding collection, imported call-target
  materialization, and tests now live in focused
  `nose-normalize/src/call_target_evidence/*` modules;
- keep imperative frontend language lowerers split by lowering concern; C, Go,
  Java, Python, Ruby, Rust, and Swift roots are thin entry points, while items,
  statements/control, expressions/calls, imports/factories, language-specific
  helpers, and tests live under focused `nose-frontend/src/<language>/*`
  modules;
- keep normalize idiom canonicalization split by proof responsibility; call
  dispatch, receiver proof, argument construction, receiver-domain evidence
  checks, map/lambda surface recognition, and tests now live in focused
  `nose-normalize/src/idioms/*` modules;
- keep value-graph control construction split by control concern; unit entry,
  guarded-return rewrites, guard/block facts, static runtime-error recognition,
  container walking, statement dispatch, loop state, loop idioms, local reductions,
  and block-return evaluation now live in focused
  `nose-normalize/src/value_graph/control/*` modules;
- keep value-graph collection recognition split by semantic surface; element/range
  values, reduction builtins, cardinality and static membership comparisons,
  map/default recognition, collection library-call adapters, HOF/lambda
  evaluation, and Rust option helpers now live in focused
  `nose-normalize/src/value_graph/collections/*` modules;
- keep value-graph canonicalization split by rewrite family; core `mk`
  interning, operand ordering, unary/binary algebraic rewrites, Phi selection
  idioms, comparison lattice laws, byte-pack recognition, constants/literal
  membership, and value-DAG reference checks now live in focused
  `nose-normalize/src/value_graph/canonicalize/*` modules;
- keep value-graph expression evaluation split by expression family; core
  dispatch, literals/free variables, binary operators, field/index access,
  calls, and structured expressions now live in focused
  `nose-normalize/src/value_graph/eval/*` modules;
- keep standard-library value recognizers split by proof surface; collection
  factories, import facts, local binding evidence, library API spans, map
  factories/access/membership, and integer min/max/clamp calls now live in
  focused `nose-normalize/src/value_graph/stdlib/*` modules;
- keep compiled semantic-pack count helpers split by ownership surface; protocol
  pack counts live under `nose-semantics/src/packs/compiled/counts/protocols.rs`
  instead of growing the count-suite root;
- keep value-graph tests as a thin suite root plus domain modules; builder,
  factory, guard, library API, membership, promise, sequence-surface, source
  evidence, and shared fixture helpers now live under
  `nose-normalize/src/value_graph/tests/`;
- keep detect unit extraction focused on root orchestration; the public unit
  model, shape/minhash feature extraction, unit timing, IL tree helpers,
  exact-fragment root dispatch, ordered effect sequences, Java self-field
  fragments, loop-effect fragments, fragment context-safety, and unit tests now
  live in focused `nose-detect/src/units/*` modules;
- keep strict exact-safety policy fail-closed but locally owned; fact collection,
  tree entry points, HoF/comprehension safety, primitive literal/sequence gates,
  static index membership, call dispatch, collection/map receivers, factory/map
  constructors, callee identity, and policy tests now live in focused
  `nose-detect/src/strict_exact/*` modules;
- keep the detect crate root and report/witness surfaces as thin APIs; detect options,
  scorer policy, public report/location models, reinvented-helper containment,
  orchestration, candidate/group construction, report ranking/path policy, report test
  suites, and graded-witness anti-unification now live in focused
  `nose-detect/src/{options,detectors,model,locations,reinvented,orchestration,candidates,report/*,witness/*}.rs`
  modules;
- move reusable semantic or detection rules toward the owning library crate
  instead of keeping them in `nose-cli`;
- split wide language and IL dispatch only around real concepts, such as
  expression lowering, declaration facts, effect evidence, or value-graph state;
- keep table-driven and cross-language tests readable by extracting shared
  fixtures only when the name explains the scenario being tested;
- keep exact-fragment CLI fixture scanning and family-selection helpers outside
  the oversized test body; shared exact-fragment test support now lives in
  `nose-cli/tests/cli/exact_fragments/support.rs`;
- keep Java `this` field exact-fragment scenarios together in their own CLI
  test module; assignment, guarded branch, body, and fluent `return this`
  fixtures now live in `nose-cli/tests/cli/exact_fragments/java_this_field.rs`;
- keep ordered branch exact-fragment matrices grouped by effect shape; ordered
  foreach-effect and mixed-effect branch fixtures now live in
  `nose-cli/tests/cli/exact_fragments/ordered_effect_branches.rs`;
- keep ordered conditional branch exact-fragment matrices grouped by control
  shape; conditional-only fixtures now live in
  `nose-cli/tests/cli/exact_fragments/ordered_conditional_branches.rs`, and
  loop-plus-conditional fixtures live in
  `nose-cli/tests/cli/exact_fragments/ordered_loop_conditional_branches.rs`;
- turn oversized integration-test files into small suite roots plus domain-named
  modules, keeping each new module under the 600-line target;
- keep CLI integration suites as thin roots that declare domain modules;
  command-surface, exact-fragment, and semantic-idiom scenarios now live under
  `nose-cli/tests/cli/{commands,exact_fragments,semantic_idioms}/`;
- lower a file budget only in the same change that makes the corresponding
  design boundary clearer.

When a large file is reduced below 600 lines, remove its entry from
`scripts/file-length-budgets.json`.
