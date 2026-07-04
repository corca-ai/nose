# Type-4 frontier priorities

This report is generated from the pinned benchmark repos by
`bench/type4/prioritize_frontier.py`. Scores combine real-code frequency,
repo/language spread, estimated implementation cost, soundness risk, scope,
and whether a frontier is already covered.

- repos analyzed: 120
- files analyzed: 60617
- max bytes per file: 512000
- matches: raw syntactic hits
- weighted: raw hits adjusted by pattern precision (`high=1.0`, `medium=0.55`, `low=0.15`)
- probe coverage: broad-probe hits already covered by extraction patterns; gaps feed the next pattern loop
- filtered: broad-probe hits rejected as overreach before coverage is scored

| rank | candidate | scope | status | score | raw | weighted | repos | languages | probe coverage | gaps | filtered |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `membership_contains` | multi-language | partially-covered | 37.64 | 23027 | 13505.5 | 105 | 7 | 100.0% | 0 | 2797 |
| 2 | `map_default_lookup` | multi-language | partially-covered | 20.30 | 4334 | 3655.4 | 73 | 7 | 100.0% | 0 | 0 |
| 3 | `collection_empty_check` | all-language | covered-current | 9.27 | 21615 | 18188.8 | 103 | 8 | 100.0% | 0 | 1 |
| 4 | `string_prefix_suffix` | all-language | covered-current | 7.25 | 6330 | 6330.0 | 98 | 7 | 100.0% | 0 | 0 |
| 5 | `null_option_presence` | all-language | covered-current | 6.51 | 127849 | 124612.6 | 99 | 7 | 100.0% | 0 | 0 |
| 6 | `own_property_guard` | language-family | covered-current | 0.65 | 819 | 819.0 | 26 | 2 | 100.0% | 0 | 0 |
| 7 | `numeric_minmax_abs` | all-language | covered-current | 0.63 | 425 | 425.0 | 15 | 1 | 100.0% | 0 | 0 |
| 8 | `property_type_guard` | language-family | covered-current | 0.43 | 450 | 450.0 | 22 | 2 | 100.0% | 0 | 0 |

## Recommended Order

1. `membership_contains`
   - why: Static literal collection membership, typed dynamic receiver membership including Python `tuple[T, ...]`, Java `Queue<T>`, Rust `VecDeque<T>`, and Python stdlib `Sequence`/`Container`/`Set` alias type facts, Python builtin `set`/`tuple`/`frozenset` factories, Python stdlib `collections.deque([...])` factories through import/alias/namespace provenance, Ruby stdlib `Set.new([...])` factories and `member?` aliases, function-local Go slice / Java `List.of` / Rust `vec!` constructed bindings, Rust std `HashSet::from`/`BTreeSet::from`/`VecDeque::from` constructed bindings, proven Set construction, static JS/TS array `.some(...)` existential, `.every(...)` absence, `.indexOf(...)` membership comparisons, `.findIndex(...)` lambda membership comparisons, and `.filter(...).length` nonempty membership / zero-count absence checks, Java literal collection factories, same-file module/static-final JS/TS/Java collection bindings, Go `slices.Contains` over package-level proven slice bindings, and Rust local immutable literal array/slice bindings are covered; typed/proven map-key membership including Python/TypeScript key-view surfaces is handled by the separate `map_key_membership` axis; substring contains, value membership, mutated or append-expanded bindings, missing imports, shadowed constructors/types/packages, untyped dynamic sets, and ambiguous receiver contains must stay distinct.
   - evidence: 23027 raw / 13505.5 weighted matches across 105 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0; filtered probe hits: 2797
   - next probe: Continue with dynamic collection/set membership only when receiver/element coordinates can be proven by imported or cross-file immutable bindings, construction facts, explicit stdlib import facts, or type facts beyond the current typed-parameter, Python builtin factory, Python stdlib deque factory, Ruby Set factory, function-local constructed binding, Python tuple, Java Queue, Rust VecDeque, Python stdlib collection alias, literal-Set, Java literal-factory, Rust std factory, same-file module-binding, Go imported-package slice-binding, and Rust local literal-binding cases; keep substring, regex, map-key, mutation, missing-import, shadowing, append-expanded construction, and unproven receiver-overloaded calls as hard boundaries.
2. `map_default_lookup`
   - why: Literal Python/Ruby map lookup, JS/TS inline/local/module Map and object defaults, typed Go/Java/Rust maps, typed TypeScript Map fallbacks, typed Python `dict`/`Mapping` fallbacks including proven stdlib `typing`/`collections.abc` map aliases, Java Map.of/Map.ofEntries literal factories, Java static-final Map.of bindings, Rust std HashMap/BTreeMap literal factories, and Go `map[string]int|string|bool|float64|*T{...}[key]` zero-value default lookups are covered; cross-file imports, richer receiver facts, untyped Python/Ruby/JS receiver defaults, remaining Go zero-value families, absent-key semantics beyond proven zero defaults, and mutation/effects remain open.
   - evidence: 4334 raw / 3655.4 weighted matches across 73 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0; filtered probe hits: 0
   - next probe: Continue with imported or cross-file Map/object defaults only when receiver/key/default coordinates can be proven by import identity, immutable binding, type facts, and whole-file mutation exclusion beyond the current inline/local/module construction, Rust std factory, and Python stdlib alias type-fact cases.

## Pattern Diagnostics

### `membership_contains`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `py_in_predicate` | python | medium | 8707 | 4788.9 | 30 |
| `java_contains_ambiguous` | java | medium | 5867 | 3226.8 | 16 |
| `ruby_membership` | ruby | medium | 2199 | 1209.5 | 18 |
| `rust_contains_ambiguous` | rust | medium | 2128 | 1170.4 | 15 |
| `java_contains_key` | java | high | 1139 | 1139.0 | 13 |
| `ts_membership_ambiguous` | typescript | medium | 1196 | 657.8 | 17 |
| `js_membership_ambiguous` | javascript | medium | 1062 | 584.1 | 28 |
| `go_map_ok` | go | high | 525 | 525.0 | 16 |
| `go_slices_contains` | go | high | 121 | 121.0 | 9 |
| `rust_contains_key` | rust | high | 83 | 83.0 | 10 |

### `map_default_lookup`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `py_get_default` | python | high | 2105 | 2105.0 | 25 |
| `go_map_lookup_ok` | go | medium | 1470 | 808.5 | 16 |
| `ruby_fetch_default` | ruby | high | 558 | 558.0 | 14 |
| `java_get_or_default` | java | high | 137 | 137.0 | 7 |
| `rust_get_unwrap_default` | rust | high | 26 | 26.0 | 5 |
| `ts_map_get_default` | typescript | medium | 36 | 19.8 | 6 |
| `js_map_get_default` | javascript | medium | 2 | 1.1 | 2 |

### `collection_empty_check`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `java_named_empty` | java | high | 5252 | 5252.0 | 18 |
| `go_len_zero` | go | high | 5103 | 5103.0 | 16 |
| `rust_named_empty` | rust | high | 2582 | 2582.0 | 15 |
| `c_len_zero` | c | medium | 3618 | 1989.9 | 22 |
| `ts_length_zero` | typescript | high | 1086 | 1086.0 | 14 |
| `py_len_zero` | python | high | 647 | 647.0 | 22 |
| `js_length_zero` | javascript | high | 423 | 423.0 | 24 |
| `py_truthy_collection` | python | low | 1838 | 275.7 | 28 |
| `ruby_length_zero` | ruby | high | 270 | 270.0 | 7 |
| `java_size_zero` | java | high | 239 | 239.0 | 12 |
| `ts_expect_length_zero` | typescript | medium | 422 | 232.1 | 4 |
| `rust_assert_len_zero` | rust | medium | 77 | 42.4 | 8 |

### `string_prefix_suffix`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `java_prefix_suffix` | java | high | 1579 | 1579.0 | 16 |
| `go_strings_prefix_suffix` | go | high | 1405 | 1405.0 | 14 |
| `py_prefix_suffix` | python | high | 941 | 941.0 | 27 |
| `ts_prefix_suffix` | typescript | high | 865 | 865.0 | 12 |
| `ruby_prefix_suffix` | ruby | high | 624 | 624.0 | 17 |
| `rust_prefix_suffix` | rust | high | 562 | 562.0 | 16 |
| `js_prefix_suffix` | javascript | high | 354 | 354.0 | 18 |

### `null_option_presence`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `go_nil_compare` | go | high | 47534 | 47534.0 | 18 |
| `c_null_compare` | c | high | 28989 | 28989.0 | 23 |
| `java_null_compare` | java | high | 25850 | 25850.0 | 18 |
| `py_none_compare` | python | high | 12398 | 12398.0 | 29 |
| `ts_nullish_compare` | typescript | high | 2338 | 2338.0 | 16 |
| `rust_option_predicate` | rust | high | 2270 | 2270.0 | 16 |
| `rust_if_let_some` | rust | medium | 4000 | 2200.0 | 16 |
| `ts_nullish_default` | typescript | medium | 2786 | 1532.3 | 16 |
| `js_nullish_compare` | javascript | high | 1278 | 1278.0 | 27 |
| `js_nullish_default` | javascript | medium | 406 | 223.3 | 15 |

### `own_property_guard`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `ts_own_property` | typescript | high | 500 | 500.0 | 15 |
| `js_own_property` | javascript | high | 319 | 319.0 | 19 |

### `numeric_minmax_abs`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `rust_numeric_method` | rust | high | 425 | 425.0 | 15 |

### `property_type_guard`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `ts_typeof_property` | typescript | high | 317 | 317.0 | 11 |
| `js_typeof_property` | javascript | high | 133 | 133.0 | 14 |


## Gap Samples

### `membership_contains`
- no uncovered broad-probe samples

### `map_default_lookup`
- no uncovered broad-probe samples

### `collection_empty_check`
- no uncovered broad-probe samples

### `string_prefix_suffix`
- no uncovered broad-probe samples

### `null_option_presence`
- no uncovered broad-probe samples

### `own_property_guard`
- no uncovered broad-probe samples

### `numeric_minmax_abs`
- no uncovered broad-probe samples

### `property_type_guard`
- no uncovered broad-probe samples


## Filtered Probe Samples

### `membership_contains`
- `antlr4/runtime/Python3/src/antlr4/IntervalSet.py:96` (python, py_membership_broad, python-for-in-iteration): return sum(len(i) for i in self.intervals)
- `antlr4/runtime/Python3/src/antlr4/LL1Analyzer.py:145` (python, py_membership_broad, python-for-in-iteration): return for t in s.transitions:
- `antlr4/runtime/Python3/src/antlr4/Parser.py:551` (python, py_membership_broad, python-for-in-iteration): return [ str(dfa) for dfa in self._interp.decisionToDFA]
- `antlr4/runtime/Python3/src/antlr4/atn/ATNConfigSet.py:117` (python, py_membership_broad, python-for-in-iteration): return set(c.state for c in self.configs)
- `antlr4/runtime/Python3/src/antlr4/atn/ATNConfigSet.py:120` (python, py_membership_broad, python-for-in-iteration): return list(cfg.semanticContext for cfg in self.configs if cfg.semanticContext!=SemanticContext.NONE)

### `map_default_lookup`
- no filtered broad-probe samples

### `collection_empty_check`
- `sympy/sympy/matrices/tests/test_sparse.py:578` (python, py_collection_emptyish, compound-length-arithmetic): assert (len(a.todok()) + len(b.todok()) - len((a + b).todok()) > 0)

### `string_prefix_suffix`
- no filtered broad-probe samples

### `null_option_presence`
- no filtered broad-probe samples

### `own_property_guard`
- no filtered broad-probe samples

### `numeric_minmax_abs`
- no filtered broad-probe samples

### `property_type_guard`
- no filtered broad-probe samples


## Audit Repo Samples

### `membership_contains`
- `guava` (dev, Java; java): 2646 raw / 1746.5 weighted
- `sympy` (heldout, Python; python): 2961 raw / 1628.5 weighted
- `sqlalchemy` (heldout, Python; python): 2102 raw / 1156.1 weighted
- `nushell` (dev, Rust; javascript, python, rust): 1359 raw / 763.6 weighted
- `scrapy` (dev, Python; python): 744 raw / 409.2 weighted

### `map_default_lookup`
- `sqlalchemy` (heldout, Python; python): 797 raw / 797.0 weighted
- `sympy` (heldout, Python; python): 610 raw / 610.0 weighted
- `rubocop` (dev, Ruby; ruby): 275 raw / 275.0 weighted
- `minio` (heldout, Go; go, python): 294 raw / 162.2 weighted
- `poetry` (dev, Python; python): 134 raw / 134.0 weighted

### `collection_empty_check`
- `guava` (dev, Java; java): 1924 raw / 1924.0 weighted
- `nats-server` (dev, Go; go): 1194 raw / 1194.0 weighted
- `nushell` (dev, Rust; rust): 1023 raw / 1019.9 weighted
- `prometheus` (dev, Go; go, typescript): 958 raw / 958.0 weighted
- `minio` (heldout, Go; go, python): 828 raw / 827.1 weighted

### `string_prefix_suffix`
- `drizzle-orm` (dev, TypeScript; javascript, typescript): 506 raw / 506.0 weighted
- `h2database` (heldout, Java; java, javascript): 434 raw / 434.0 weighted
- `nushell` (dev, Rust; rust): 307 raw / 307.0 weighted
- `esbuild` (heldout, Go; go, javascript, typescript): 258 raw / 258.0 weighted
- `hugo` (dev, Go; go, javascript): 257 raw / 257.0 weighted

### `null_option_presence`
- `vim` (heldout, C; c, python): 13453 raw / 13453.0 weighted
- `nats-server` (dev, Go; go): 12704 raw / 12704.0 weighted
- `minio` (heldout, Go; go, python): 10023 raw / 10023.0 weighted
- `prometheus` (dev, Go; go, javascript, typescript): 5450 raw / 5446.9 weighted
- `etcd` (heldout, Go; go): 5229 raw / 5229.0 weighted

### `own_property_guard`
- `esbuild` (heldout, Go; javascript, typescript): 147 raw / 147.0 weighted
- `drizzle-orm` (dev, TypeScript; javascript, typescript): 139 raw / 139.0 weighted
- `trpc` (heldout, TypeScript; typescript): 116 raw / 116.0 weighted
- `jest` (dev, TypeScript; javascript, typescript): 80 raw / 80.0 weighted
- `prettier` (dev, TypeScript; javascript, typescript): 54 raw / 54.0 weighted

### `numeric_minmax_abs`
- `nushell` (dev, Rust; rust): 113 raw / 113.0 weighted
- `image` (dev, Rust; rust): 90 raw / 90.0 weighted
- `meilisearch` (heldout, Rust; rust): 61 raw / 61.0 weighted
- `alacritty` (dev, Rust; rust): 44 raw / 44.0 weighted
- `sled` (heldout, Rust; rust): 42 raw / 42.0 weighted

### `property_type_guard`
- `jest` (dev, TypeScript; javascript, typescript): 89 raw / 89.0 weighted
- `drizzle-orm` (dev, TypeScript; typescript): 76 raw / 76.0 weighted
- `prettier` (dev, TypeScript; javascript): 60 raw / 60.0 weighted
- `pixijs` (heldout, TypeScript; javascript, typescript): 58 raw / 58.0 weighted
- `zod` (dev, TypeScript; typescript): 41 raw / 41.0 weighted


## Extraction Samples

### `membership_contains`
- `alacritty/alacritty/src/cli.rs:137` (rust, rust_contains_ambiguous): Some((_, instance)) if instance.contains(',') => {
- `alacritty/alacritty/src/config/bindings.rs:60` (rust, rust_contains_ambiguous): && mode.contains(self.mode)
- `alacritty/alacritty/src/config/bindings.rs:775` (rust, rust_contains_ambiguous): binding_mode.set(BindingMode::APP_CURSOR, mode.contains(TermMode::APP_CURSOR));
- `alacritty/alacritty/src/config/bindings.rs:776` (rust, rust_contains_ambiguous): binding_mode.set(BindingMode::APP_KEYPAD, mode.contains(TermMode::APP_KEYPAD));
- `alacritty/alacritty/src/config/bindings.rs:777` (rust, rust_contains_ambiguous): binding_mode.set(BindingMode::ALT_SCREEN, mode.contains(TermMode::ALT_SCREEN));

### `map_default_lookup`
- `alacritty/alacritty_terminal/src/tty/mod.rs:114` (rust, rust_get_unwrap_default): let first = terminfo.get(..1).unwrap_or_default();
- `antlr4/runtime/Go/antlr/v4/parser_atn_simulator.go:1010` (go, go_map_lookup_ok): if _, ok := visited[currConfig]; ok {
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:550` (go, go_map_lookup_ok): if iop, ok := rewrites[j].(*InsertBeforeOp); ok {
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:568` (go, go_map_lookup_ok): if prevop, ok := rewrites[j].(*ReplaceOp); ok {
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:595` (go, go_map_lookup_ok): _, iok := rewrites[i].(*InsertBeforeOp)

### `collection_empty_check`
- `alacritty/alacritty/src/cli.rs:401` (rust, rust_named_empty): if self.config_options.is_empty() {
- `alacritty/alacritty/src/config/bindings.rs:71` (rust, rust_named_empty): let selfmode = if self.mode.is_empty() { BindingMode::all() } else { self.mode };
- `alacritty/alacritty/src/config/bindings.rs:72` (rust, rust_named_empty): let bindingmode = if binding.mode.is_empty() { BindingMode::all() } else { binding.mode };
- `alacritty/alacritty/src/config/mod.rs:221` (rust, rust_named_empty): if (extension == "yaml" || extension == "yml") && !contents.trim().is_empty() {
- `alacritty/alacritty/src/config/mod.rs:294` (rust, rust_named_empty): if !imports.is_empty() && recursion_limit == 0 {

### `string_prefix_suffix`
- `alacritty/alacritty/src/config/bindings.rs:736` (rust, rust_prefix_suffix): _ if keycode.starts_with("Dead") => {
- `alacritty/alacritty/src/config/mod.rs:215` (rust, rust_prefix_suffix): if contents.starts_with('\u{FEFF}') {
- `alacritty/alacritty/src/display/color.rs:287` (rust, rust_prefix_suffix): let chars = if s.starts_with("0x") && s.len() == 8 {
- `alacritty/alacritty/src/display/color.rs:289` (rust, rust_prefix_suffix): } else if s.starts_with('#') && s.len() == 7 {
- `alacritty/alacritty/src/polling/ipc.rs:197` (rust, rust_prefix_suffix): .filter(|file| file.starts_with(&socket_prefix) && file.ends_with(".sock"))

### `null_option_presence`
- `alacritty/alacritty/build.rs:10` (rust, rust_if_let_some): if let Some(commit_hash) = commit_hash() {
- `alacritty/alacritty/src/cli.rs:95` (rust, rust_option_predicate): if self.socket.is_some() {
- `alacritty/alacritty/src/cli.rs:180` (rust, rust_if_let_some): if let Some(working_directory) = &self.working_directory {
- `alacritty/alacritty/src/cli.rs:188` (rust, rust_if_let_some): if let Some(command) = self.command() {
- `alacritty/alacritty/src/cli.rs:224` (rust, rust_if_let_some): if let Some(title) = &self.title {

### `own_property_guard`
- `axios/lib/adapters/fetch.js:348` (javascript, js_own_property): const isCredentialsSupported = isRequestSupported && 'credentials' in Request.prototype;
- `axios/lib/adapters/xhr.js:48` (javascript, js_own_property): 'getAllResponseHeaders' in request && request.getAllResponseHeaders()
- `axios/lib/adapters/xhr.js:79` (javascript, js_own_property): if ('onloadend' in request) {
- `axios/lib/adapters/xhr.js:159` (javascript, js_own_property): if ('setRequestHeader' in request) {
- `axios/lib/helpers/validator.js:91` (javascript, js_own_property): const validator = Object.prototype.hasOwnProperty.call(schema, opt) ? schema[opt] : undefined;

### `numeric_minmax_abs`
- `alacritty/alacritty/src/display/bell.rs:75` (rust, rust_numeric_method): let time = (elapsed_f / duration_f).min(1.0);
- `alacritty/alacritty/src/display/cursor.rs:25` (rust, rust_numeric_method): let thickness = (thickness * width).round().max(1.);
- `alacritty/alacritty/src/display/damage.rs:124` (rust, rust_numeric_method): || selection.start.line.0.abs() < display_offset - last_visible_line
- `alacritty/alacritty/src/display/damage.rs:239` (rust, rust_numeric_method): rect.x = (rect.x - size_info.cell_width() as i32).max(0);
- `alacritty/alacritty/src/display/damage.rs:241` (rust, rust_numeric_method): (size_info.width() as i32 - rect.x).max(0),

### `property_type_guard`
- `axios/lib/adapters/fetch.js:458` (javascript, js_typeof_property): if (typeof responseData.byteLength === 'number') {
- `axios/lib/adapters/fetch.js:460` (javascript, js_typeof_property): } else if (typeof responseData.size === 'number') {
- `axios/lib/core/Axios.js:162` (javascript, js_typeof_property): if (typeof interceptor.runWhen === 'function' && interceptor.runWhen(config) === false) {
- `axios/lib/helpers/estimateDataURLDecodedBytes.js:72` (javascript, js_typeof_property): if (typeof Buffer !== 'undefined' && typeof Buffer.byteLength === 'function') {
- `axios/lib/platform/common/utils.js:40` (javascript, js_typeof_property): typeof self.importScripts === 'function'
