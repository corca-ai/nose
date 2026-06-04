# Type-4 frontier priorities

This report is generated from the pinned benchmark repos by
`bench/type4/prioritize_frontier.py`. Scores combine real-code frequency,
repo/language spread, estimated implementation cost, soundness risk, scope,
and whether a frontier is already covered.

- repos scanned: 105
- files scanned: 59515
- max bytes per file: 512000
- matches: raw syntactic hits
- weighted: raw hits adjusted by pattern precision (`high=1.0`, `medium=0.55`, `low=0.15`)
- probe coverage: broad-probe hits already covered by extraction patterns; gaps feed the next pattern loop
- filtered: broad-probe hits rejected as overreach before coverage is scored

| rank | candidate | scope | status | score | raw | weighted | repos | languages | probe coverage | gaps | filtered |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `membership_contains` | multi-language | partially-covered | 36.54 | 22979 | 13478.1 | 99 | 7 | 100.0% | 0 | 2798 |
| 2 | `map_default_lookup` | multi-language | partially-covered | 20.30 | 4319 | 3645.3 | 73 | 7 | 100.0% | 0 | 0 |
| 3 | `collection_empty_check` | all-language | covered-current | 9.04 | 21562 | 18145.0 | 98 | 8 | 100.0% | 0 | 1 |
| 4 | `string_prefix_suffix` | all-language | covered-current | 7.20 | 6174 | 6174.0 | 97 | 7 | 100.0% | 0 | 0 |
| 5 | `null_option_presence` | all-language | covered-current | 6.34 | 126057 | 122957.4 | 94 | 7 | 100.0% | 0 | 0 |
| 6 | `numeric_minmax_abs` | all-language | partially-covered | 5.14 | 425 | 425.0 | 15 | 1 | 100.0% | 0 | 0 |
| 7 | `own_property_guard` | language-family | covered-current | 0.60 | 764 | 764.0 | 23 | 2 | 100.0% | 0 | 0 |
| 8 | `property_type_guard` | language-family | covered-current | 0.40 | 435 | 435.0 | 19 | 2 | 100.0% | 0 | 0 |

## Recommended Order

1. `membership_contains`
   - why: Static literal collection membership and explicitly typed dynamic collection membership are covered; substring contains, map-key membership, untyped dynamic sets, and ambiguous receiver contains must stay distinct.
   - evidence: 22979 raw / 13478.1 weighted matches across 99 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0; filtered probe hits: 2798
   - next probe: Continue with dynamic set membership only when receiver/key coordinates can be proven by type, construction, or local immutable binding facts; keep substring, regex, and unproven receiver-overloaded calls as hard boundaries.
2. `map_default_lookup`
   - why: Literal Python/Ruby map lookup with fallback is covered; typed maps, object/map APIs, absent-key semantics, and mutation/effects remain open.
   - evidence: 4319 raw / 3645.3 weighted matches across 73 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0; filtered probe hits: 0
   - next probe: Continue with JS/TS object-or-Map defaults, then typed Go/Java/Rust maps only when receiver/key/default coordinates are provable.
3. `numeric_minmax_abs`
   - why: Scalar min/max/abs expression facts are covered for C, Go, Java, JavaScript/TypeScript, Python, Ruby, and embedded script surfaces; Rust numeric methods remain the next compact strict slice.
   - evidence: 425 raw / 425.0 weighted matches across 15 repos and 1 languages (rust)
   - probe coverage: 100.0%; uncovered probe hits: 0; filtered probe hits: 0
   - next probe: Generate Rust scalar `.abs()`, `.min()`, and `.max()` positives with sign/order and wrong-value hard negatives.

## Pattern Diagnostics

### `membership_contains`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `py_in_predicate` | python | medium | 8705 | 4787.8 | 29 |
| `java_contains_ambiguous` | java | medium | 5867 | 3226.8 | 16 |
| `ruby_membership` | ruby | medium | 2195 | 1207.2 | 16 |
| `rust_contains_ambiguous` | rust | medium | 2128 | 1170.4 | 15 |
| `java_contains_key` | java | high | 1139 | 1139.0 | 13 |
| `ts_membership_ambiguous` | typescript | medium | 1166 | 641.3 | 17 |
| `js_membership_ambiguous` | javascript | medium | 1052 | 578.6 | 25 |
| `go_map_ok` | go | high | 523 | 523.0 | 16 |
| `go_slices_contains` | go | high | 121 | 121.0 | 9 |
| `rust_contains_key` | rust | high | 83 | 83.0 | 10 |

### `map_default_lookup`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `py_get_default` | python | high | 2101 | 2101.0 | 25 |
| `go_map_lookup_ok` | go | medium | 1459 | 802.5 | 16 |
| `ruby_fetch_default` | ruby | high | 558 | 558.0 | 14 |
| `java_get_or_default` | java | high | 137 | 137.0 | 7 |
| `rust_get_unwrap_default` | rust | high | 26 | 26.0 | 5 |
| `ts_map_get_default` | typescript | medium | 36 | 19.8 | 6 |
| `js_map_get_default` | javascript | medium | 2 | 1.1 | 2 |

### `collection_empty_check`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `java_named_empty` | java | high | 5252 | 5252.0 | 18 |
| `go_len_zero` | go | high | 5075 | 5075.0 | 16 |
| `rust_named_empty` | rust | high | 2582 | 2582.0 | 15 |
| `c_len_zero` | c | medium | 3605 | 1982.8 | 21 |
| `ts_length_zero` | typescript | high | 1086 | 1086.0 | 14 |
| `py_len_zero` | python | high | 649 | 649.0 | 22 |
| `js_length_zero` | javascript | high | 413 | 413.0 | 21 |
| `py_truthy_collection` | python | low | 1834 | 275.1 | 27 |
| `ruby_length_zero` | ruby | high | 270 | 270.0 | 7 |
| `java_size_zero` | java | high | 239 | 239.0 | 12 |
| `ts_expect_length_zero` | typescript | medium | 422 | 232.1 | 4 |
| `rust_assert_len_zero` | rust | medium | 77 | 42.4 | 8 |

### `string_prefix_suffix`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `java_prefix_suffix` | java | high | 1579 | 1579.0 | 16 |
| `go_strings_prefix_suffix` | go | high | 1391 | 1391.0 | 14 |
| `py_prefix_suffix` | python | high | 952 | 952.0 | 26 |
| `ts_prefix_suffix` | typescript | high | 715 | 715.0 | 12 |
| `ruby_prefix_suffix` | ruby | high | 624 | 624.0 | 17 |
| `rust_prefix_suffix` | rust | high | 559 | 559.0 | 16 |
| `js_prefix_suffix` | javascript | high | 354 | 354.0 | 18 |

### `null_option_presence`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `go_nil_compare` | go | high | 47075 | 47075.0 | 18 |
| `c_null_compare` | c | high | 28917 | 28917.0 | 22 |
| `java_null_compare` | java | high | 25129 | 25129.0 | 18 |
| `py_none_compare` | python | high | 12417 | 12417.0 | 28 |
| `rust_option_predicate` | rust | high | 2270 | 2270.0 | 16 |
| `rust_if_let_some` | rust | medium | 3998 | 2198.9 | 16 |
| `ts_nullish_compare` | typescript | high | 2088 | 2088.0 | 16 |
| `ts_nullish_default` | typescript | medium | 2484 | 1366.2 | 16 |
| `js_nullish_compare` | javascript | high | 1273 | 1273.0 | 24 |
| `js_nullish_default` | javascript | medium | 406 | 223.3 | 15 |

### `numeric_minmax_abs`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `rust_numeric_method` | rust | high | 425 | 425.0 | 15 |

### `own_property_guard`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `ts_own_property` | typescript | high | 450 | 450.0 | 15 |
| `js_own_property` | javascript | high | 314 | 314.0 | 16 |

### `property_type_guard`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `ts_typeof_property` | typescript | high | 307 | 307.0 | 11 |
| `js_typeof_property` | javascript | high | 128 | 128.0 | 11 |


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

### `numeric_minmax_abs`
- no uncovered broad-probe samples

### `own_property_guard`
- no uncovered broad-probe samples

### `property_type_guard`
- no uncovered broad-probe samples


## Filtered Probe Samples

### `membership_contains`
- `antlr4/runtime/Python3/src/antlr4/Parser.py:551` (python, py_membership_broad, python-for-in-iteration): return [ str(dfa) for dfa in self._interp.decisionToDFA]
- `antlr4/runtime/Python3/src/antlr4/LL1Analyzer.py:145` (python, py_membership_broad, python-for-in-iteration): return for t in s.transitions:
- `antlr4/runtime/Python3/src/antlr4/IntervalSet.py:96` (python, py_membership_broad, python-for-in-iteration): return sum(len(i) for i in self.intervals)
- `antlr4/runtime/Python3/src/antlr4/tree/Trees.py:64` (python, py_membership_broad, python-for-in-iteration): return [ t.getChild(i) for i in range(0, t.getChildCount()) ]
- `antlr4/runtime/Python3/src/antlr4/dfa/DFAState.py:89` (python, py_membership_broad, python-for-in-iteration): return set(cfg.alt for cfg in self.configs) or None

### `map_default_lookup`
- no filtered broad-probe samples

### `collection_empty_check`
- `sympy/sympy/matrices/tests/test_sparse.py:578` (python, py_collection_emptyish, compound-length-arithmetic): assert (len(a.todok()) + len(b.todok()) - len((a + b).todok()) > 0)

### `string_prefix_suffix`
- no filtered broad-probe samples

### `null_option_presence`
- no filtered broad-probe samples

### `numeric_minmax_abs`
- no filtered broad-probe samples

### `own_property_guard`
- no filtered broad-probe samples

### `property_type_guard`
- no filtered broad-probe samples


## Audit Repo Samples

### `membership_contains`
- `guava` (dev, Java; java): 2646 raw / 1746.5 weighted
- `sympy` (heldout, Python; python): 2961 raw / 1628.5 weighted
- `sqlalchemy` (heldout, Python; python): 2091 raw / 1150.0 weighted
- `nushell` (dev, Rust; javascript, python, rust): 1359 raw / 763.6 weighted
- `scrapy` (dev, Python; python): 744 raw / 409.2 weighted

### `map_default_lookup`
- `sqlalchemy` (heldout, Python; python): 796 raw / 796.0 weighted
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
- `nats-server` (dev, Go; go): 244 raw / 244.0 weighted

### `null_option_presence`
- `vim` (heldout, C; c, python): 13453 raw / 13453.0 weighted
- `nats-server` (dev, Go; go): 12704 raw / 12704.0 weighted
- `minio` (heldout, Go; go, python): 10023 raw / 10023.0 weighted
- `prometheus` (dev, Go; go, javascript, typescript): 5464 raw / 5460.9 weighted
- `etcd` (heldout, Go; go): 5229 raw / 5229.0 weighted

### `numeric_minmax_abs`
- `nushell` (dev, Rust; rust): 113 raw / 113.0 weighted
- `image` (dev, Rust; rust): 90 raw / 90.0 weighted
- `meilisearch` (heldout, Rust; rust): 61 raw / 61.0 weighted
- `alacritty` (dev, Rust; rust): 44 raw / 44.0 weighted
- `sled` (heldout, Rust; rust): 42 raw / 42.0 weighted

### `own_property_guard`
- `esbuild` (heldout, Go; javascript, typescript): 147 raw / 147.0 weighted
- `drizzle-orm` (dev, TypeScript; javascript, typescript): 139 raw / 139.0 weighted
- `jest` (dev, TypeScript; javascript, typescript): 80 raw / 80.0 weighted
- `trpc` (heldout, TypeScript; typescript): 66 raw / 66.0 weighted
- `prettier` (dev, TypeScript; javascript, typescript): 54 raw / 54.0 weighted

### `property_type_guard`
- `jest` (dev, TypeScript; javascript, typescript): 89 raw / 89.0 weighted
- `drizzle-orm` (dev, TypeScript; typescript): 76 raw / 76.0 weighted
- `prettier` (dev, TypeScript; javascript): 60 raw / 60.0 weighted
- `pixijs` (heldout, TypeScript; javascript, typescript): 58 raw / 58.0 weighted
- `zod` (dev, TypeScript; typescript): 41 raw / 41.0 weighted


## Extraction Samples

### `membership_contains`
- `alacritty/alacritty/src/window_context.rs:542` (rust, rust_contains_ambiguous): let origin_at_bottom = if terminal.mode().contains(TermMode::VI) {
- `alacritty/alacritty/src/message_bar.rs:188` (rust, rust_contains_ambiguous): self.messages.contains(message)
- `alacritty/alacritty/src/logging.rs:188` (rust, rust_contains_ambiguous): _ => ALLOWED_TARGETS.contains(&target) || extra_log_targets().iter().any(|t| t == target),
- `alacritty/alacritty/src/event.rs:720` (rust, rust_contains_ambiguous): let vi_mode = self.terminal.mode().contains(TermMode::VI);
- `alacritty/alacritty/src/event.rs:780` (rust, rust_contains_ambiguous): if self.terminal.mode().contains(TermMode::VI) && !self.search_active() {

### `map_default_lookup`
- `alacritty/alacritty_terminal/src/tty/mod.rs:114` (rust, rust_get_unwrap_default): let first = terminfo.get(..1).unwrap_or_default();
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:550` (go, go_map_lookup_ok): if iop, ok := rewrites[j].(*InsertBeforeOp); ok {
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:568` (go, go_map_lookup_ok): if prevop, ok := rewrites[j].(*ReplaceOp); ok {
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:595` (go, go_map_lookup_ok): _, iok := rewrites[i].(*InsertBeforeOp)
- `antlr4/runtime/Go/antlr/v4/tokenstream_rewriter.go:596` (go, go_map_lookup_ok): _, aok := rewrites[i].(*InsertAfterOp)

### `collection_empty_check`
- `alacritty/alacritty/src/window_context.rs:413` (rust, rust_named_empty): if self.event_queue.is_empty() {
- `alacritty/alacritty/src/message_bar.rs:54` (rust, rust_named_empty): || (lines.is_empty()
- `alacritty/alacritty/src/message_bar.rs:148` (rust, rust_named_empty): self.messages.is_empty()
- `alacritty/alacritty/src/message_bar.rs:255` (rust, rust_assert_len_zero): assert_eq!(lines.len(), 0);
- `alacritty/alacritty/src/event.rs:346` (rust, rust_named_empty): if !window_context.message_buffer.is_empty() {

### `string_prefix_suffix`
- `alacritty/alacritty/src/polling/ipc.rs:197` (rust, rust_prefix_suffix): .filter(|file| file.starts_with(&socket_prefix) && file.ends_with(".sock"))
- `alacritty/alacritty/src/polling/ipc.rs:197` (rust, rust_prefix_suffix): .filter(|file| file.starts_with(&socket_prefix) && file.ends_with(".sock"))
- `alacritty/alacritty/src/config/mod.rs:215` (rust, rust_prefix_suffix): if contents.starts_with('\u{FEFF}') {
- `alacritty/alacritty/src/config/bindings.rs:736` (rust, rust_prefix_suffix): _ if keycode.starts_with("Dead") => {
- `alacritty/alacritty/src/display/color.rs:287` (rust, rust_prefix_suffix): let chars = if s.starts_with("0x") && s.len() == 8 {

### `null_option_presence`
- `alacritty/alacritty/build.rs:10` (rust, rust_if_let_some): if let Some(commit_hash) = commit_hash() {
- `alacritty/alacritty/src/window_context.rs:138` (rust, rust_option_predicate): let tabbed = options.window_tabbing_id.is_some();
- `alacritty/alacritty/src/window_context.rs:178` (rust, rust_option_predicate): let preserve_title = options.window_identity.title.is_some();
- `alacritty/alacritty/src/window_context.rs:427` (rust, rust_option_predicate): let old_is_searching = self.search_state.history_index.is_some();
- `alacritty/alacritty/src/window_context.rs:550` (rust, rust_option_predicate): let new_is_searching = search_state.history_index.is_some();

### `numeric_minmax_abs`
- `alacritty/alacritty/src/window_context.rs:271` (rust, rust_numeric_method): if (old_config.cursor.thickness() - self.config.cursor.thickness()).abs() > f32::EPSILON {
- `alacritty/alacritty/src/event.rs:1747` (rust, rust_numeric_method): let font_delta = (delta.abs() / FONT_SIZE_STEP).floor() * FONT_SIZE_STEP * delta.signum();
- `alacritty/alacritty/src/renderer/rects.rs:98` (rust, rust_numeric_method): Flags::UNDERCURL => (metrics.descent, metrics.descent.abs(), RectKind::Undercurl),
- `alacritty/alacritty/src/renderer/rects.rs:104` (rust, rust_numeric_method): (metrics.descent, metrics.descent.abs(), RectKind::DottedUnderline)
- `alacritty/alacritty/src/renderer/rects.rs:136` (rust, rust_numeric_method): thickness = thickness.max(1.);

### `own_property_guard`
- `axios/tests/unit/prototypePollution.test.js:71` (javascript, js_own_property): assert.strictEqual(result.hasOwnProperty('__proto__'), false);
- `axios/tests/unit/prototypePollution.test.js:78` (javascript, js_own_property): assert.strictEqual(result.hasOwnProperty('constructor'), false);
- `axios/tests/unit/prototypePollution.test.js:85` (javascript, js_own_property): assert.strictEqual(result.hasOwnProperty('prototype'), false);
- `axios/tests/unit/prototypePollution.test.js:101` (javascript, js_own_property): assert.strictEqual(result.headers.hasOwnProperty('__proto__'), false);
- `axios/tests/unit/prototypePollution.test.js:117` (javascript, js_own_property): assert.strictEqual(result.headers.hasOwnProperty('constructor'), false);

### `property_type_guard`
- `axios/tests/smoke/esm/tests/fetch.smoke.test.js:52` (javascript, js_typeof_property): isRequest && typeof input.clone === 'function'
- `axios/tests/smoke/cjs/tests/fetch.smoke.test.cjs:55` (javascript, js_typeof_property): isRequest && typeof input.clone === 'function'
- `axios/tests/unit/adapters/http.test.js:730` (javascript, js_typeof_property): const isZstdSupported = typeof zlib.createZstdDecompress === 'function' &&
- `axios/tests/unit/adapters/http.test.js:731` (javascript, js_typeof_property): typeof zlib.zstdCompress === 'function';
- `axios/tests/module/cjs/tests/helpers/fixture.cjs:19` (javascript, js_typeof_property): if (typeof fs.rmSync === 'function') {
