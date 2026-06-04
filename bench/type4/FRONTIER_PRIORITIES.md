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

| rank | candidate | scope | status | score | raw | weighted | repos | languages | probe coverage | gaps |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | `string_prefix_suffix` | all-language | open | 89.94 | 6174 | 6174.0 | 97 | 7 | 100.0% | 0 |
| 2 | `numeric_minmax_abs` | all-language | partially-covered | 64.36 | 7037 | 6750.8 | 93 | 8 | 100.0% | 0 |
| 3 | `membership_contains` | multi-language | open | 56.85 | 25776 | 15016.5 | 99 | 7 | <100% | 1 |
| 4 | `null_option_presence` | all-language | partially-covered | 51.52 | 126057 | 122957.4 | 94 | 7 | 100.0% | 0 |
| 5 | `map_default_lookup` | multi-language | open | 31.23 | 4319 | 3645.3 | 73 | 7 | 100.0% | 0 |
| 6 | `collection_empty_check` | all-language | covered-current | 9.04 | 21562 | 18145.0 | 98 | 8 | <100% | 1 |
| 7 | `property_type_guard` | language-family | open | 5.01 | 435 | 435.0 | 19 | 2 | 100.0% | 0 |
| 8 | `own_property_guard` | language-family | covered-current | 0.60 | 764 | 764.0 | 23 | 2 | 100.0% | 0 |

## Recommended Order

1. `string_prefix_suffix`
   - why: The API names differ by language but the strict predicate coordinate is simple.
   - evidence: 6174 raw / 6174.0 weighted matches across 97 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0
   - next probe: Lower case-sensitive starts-with/ends-with calls to prefix/suffix facts; keep regex, contains, and case-folding boundaries.
2. `membership_contains`
   - why: Common but semantically overloaded: substring, list membership, map key membership, and set membership must stay distinct.
   - evidence: 25776 raw / 15016.5 weighted matches across 99 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: <100%; uncovered probe hits: 1
   - next probe: Start with static set/list membership only; keep substring, regex, and map-key boundaries separate.
3. `map_default_lookup`
   - why: Potentially high value, but absent-key semantics and mutation/effects vary heavily.
   - evidence: 4319 raw / 3645.3 weighted matches across 73 repos and 7 languages (go, java, javascript, python, ruby, rust, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0
   - next probe: Start with literal immutable maps and static keys; hard-negative missing-key/default-value changes.
4. `property_type_guard`
   - why: Very frequent in JS-family repos, but the scope is narrow and should wait behind broader axes.
   - evidence: 435 raw / 435.0 weighted matches across 19 repos and 2 languages (javascript, typescript)
   - probe coverage: 100.0%; uncovered probe hits: 0
   - next probe: Generate `typeof obj.field === <type>` variants with dynamic-key and shadowing boundaries.

## Pattern Diagnostics

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

### `numeric_minmax_abs`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `py_minmax_abs` | python | high | 2676 | 2676.0 | 23 |
| `java_math_minmax_abs` | java | high | 2261 | 2261.0 | 15 |
| `ts_math_minmax_abs` | typescript | high | 482 | 482.0 | 13 |
| `rust_numeric_method` | rust | high | 425 | 425.0 | 15 |
| `c_numeric_builtin` | c | high | 296 | 296.0 | 11 |
| `go_builtin_minmax` | go | medium | 388 | 213.4 | 13 |
| `js_math_minmax_abs` | javascript | high | 133 | 133.0 | 20 |
| `go_math_minmax_abs` | go | high | 96 | 96.0 | 8 |
| `c_minmax_macro` | c | medium | 142 | 78.1 | 10 |
| `ruby_array_minmax` | ruby | medium | 106 | 58.3 | 12 |
| `ruby_abs` | ruby | high | 32 | 32.0 | 8 |

### `membership_contains`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `py_in_predicate` | python | medium | 11502 | 6326.1 | 30 |
| `java_contains_ambiguous` | java | medium | 5867 | 3226.8 | 16 |
| `ruby_membership` | ruby | medium | 2195 | 1207.2 | 16 |
| `rust_contains_ambiguous` | rust | medium | 2128 | 1170.4 | 15 |
| `java_contains_key` | java | high | 1139 | 1139.0 | 13 |
| `ts_membership_ambiguous` | typescript | medium | 1166 | 641.3 | 17 |
| `js_membership_ambiguous` | javascript | medium | 1052 | 578.6 | 25 |
| `go_map_ok` | go | high | 523 | 523.0 | 16 |
| `go_slices_contains` | go | high | 121 | 121.0 | 9 |
| `rust_contains_key` | rust | high | 83 | 83.0 | 10 |

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

### `property_type_guard`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `ts_typeof_property` | typescript | high | 307 | 307.0 | 11 |
| `js_typeof_property` | javascript | high | 128 | 128.0 | 11 |

### `own_property_guard`

| pattern | language | precision | raw | weighted | repos |
|---|---|---|---:|---:|---:|
| `ts_own_property` | typescript | high | 450 | 450.0 | 15 |
| `js_own_property` | javascript | high | 314 | 314.0 | 16 |


## Gap Samples

### `string_prefix_suffix`
- no uncovered broad-probe samples

### `numeric_minmax_abs`
- no uncovered broad-probe samples

### `membership_contains`
- `sympy/sympy/core/function.py:1947` (python, py_membership_broad): if isinstance(expr, array_types) or any(isinstance(i[0], array_types) if isinstance(i, (tuple, list, Tuple)) else isinstance(i, array_types) for i in variables):

### `null_option_presence`
- no uncovered broad-probe samples

### `map_default_lookup`
- no uncovered broad-probe samples

### `collection_empty_check`
- `sympy/sympy/matrices/tests/test_sparse.py:578` (python, py_collection_emptyish): assert (len(a.todok()) + len(b.todok()) - len((a + b).todok()) > 0)

### `property_type_guard`
- no uncovered broad-probe samples

### `own_property_guard`
- no uncovered broad-probe samples


## Extraction Samples

### `string_prefix_suffix`
- `alacritty/alacritty/src/polling/ipc.rs:197` (rust, rust_prefix_suffix): .filter(|file| file.starts_with(&socket_prefix) && file.ends_with(".sock"))
- `alacritty/alacritty/src/polling/ipc.rs:197` (rust, rust_prefix_suffix): .filter(|file| file.starts_with(&socket_prefix) && file.ends_with(".sock"))
- `alacritty/alacritty/src/config/mod.rs:215` (rust, rust_prefix_suffix): if contents.starts_with('\u{FEFF}') {
- `alacritty/alacritty/src/config/bindings.rs:736` (rust, rust_prefix_suffix): _ if keycode.starts_with("Dead") => {
- `alacritty/alacritty/src/display/color.rs:287` (rust, rust_prefix_suffix): let chars = if s.starts_with("0x") && s.len() == 8 {

### `numeric_minmax_abs`
- `alacritty/alacritty/src/window_context.rs:271` (rust, rust_numeric_method): if (old_config.cursor.thickness() - self.config.cursor.thickness()).abs() > f32::EPSILON {
- `alacritty/alacritty/src/event.rs:1747` (rust, rust_numeric_method): let font_delta = (delta.abs() / FONT_SIZE_STEP).floor() * FONT_SIZE_STEP * delta.signum();
- `alacritty/alacritty/src/renderer/rects.rs:98` (rust, rust_numeric_method): Flags::UNDERCURL => (metrics.descent, metrics.descent.abs(), RectKind::Undercurl),
- `alacritty/alacritty/src/renderer/rects.rs:104` (rust, rust_numeric_method): (metrics.descent, metrics.descent.abs(), RectKind::DottedUnderline)
- `alacritty/alacritty/src/renderer/rects.rs:136` (rust, rust_numeric_method): thickness = thickness.max(1.);

### `membership_contains`
- `alacritty/alacritty/src/window_context.rs:542` (rust, rust_contains_ambiguous): let origin_at_bottom = if terminal.mode().contains(TermMode::VI) {
- `alacritty/alacritty/src/message_bar.rs:188` (rust, rust_contains_ambiguous): self.messages.contains(message)
- `alacritty/alacritty/src/logging.rs:188` (rust, rust_contains_ambiguous): _ => ALLOWED_TARGETS.contains(&target) || extra_log_targets().iter().any(|t| t == target),
- `alacritty/alacritty/src/event.rs:720` (rust, rust_contains_ambiguous): let vi_mode = self.terminal.mode().contains(TermMode::VI);
- `alacritty/alacritty/src/event.rs:780` (rust, rust_contains_ambiguous): if self.terminal.mode().contains(TermMode::VI) && !self.search_active() {

### `null_option_presence`
- `alacritty/alacritty/build.rs:10` (rust, rust_if_let_some): if let Some(commit_hash) = commit_hash() {
- `alacritty/alacritty/src/window_context.rs:138` (rust, rust_option_predicate): let tabbed = options.window_tabbing_id.is_some();
- `alacritty/alacritty/src/window_context.rs:178` (rust, rust_option_predicate): let preserve_title = options.window_identity.title.is_some();
- `alacritty/alacritty/src/window_context.rs:427` (rust, rust_option_predicate): let old_is_searching = self.search_state.history_index.is_some();
- `alacritty/alacritty/src/window_context.rs:550` (rust, rust_option_predicate): let new_is_searching = search_state.history_index.is_some();

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

### `property_type_guard`
- `axios/tests/smoke/esm/tests/fetch.smoke.test.js:52` (javascript, js_typeof_property): isRequest && typeof input.clone === 'function'
- `axios/tests/smoke/cjs/tests/fetch.smoke.test.cjs:55` (javascript, js_typeof_property): isRequest && typeof input.clone === 'function'
- `axios/tests/unit/adapters/http.test.js:730` (javascript, js_typeof_property): const isZstdSupported = typeof zlib.createZstdDecompress === 'function' &&
- `axios/tests/unit/adapters/http.test.js:731` (javascript, js_typeof_property): typeof zlib.zstdCompress === 'function';
- `axios/tests/module/cjs/tests/helpers/fixture.cjs:19` (javascript, js_typeof_property): if (typeof fs.rmSync === 'function') {

### `own_property_guard`
- `axios/tests/unit/prototypePollution.test.js:71` (javascript, js_own_property): assert.strictEqual(result.hasOwnProperty('__proto__'), false);
- `axios/tests/unit/prototypePollution.test.js:78` (javascript, js_own_property): assert.strictEqual(result.hasOwnProperty('constructor'), false);
- `axios/tests/unit/prototypePollution.test.js:85` (javascript, js_own_property): assert.strictEqual(result.hasOwnProperty('prototype'), false);
- `axios/tests/unit/prototypePollution.test.js:101` (javascript, js_own_property): assert.strictEqual(result.headers.hasOwnProperty('__proto__'), false);
- `axios/tests/unit/prototypePollution.test.js:117` (javascript, js_own_property): assert.strictEqual(result.headers.hasOwnProperty('constructor'), false);
