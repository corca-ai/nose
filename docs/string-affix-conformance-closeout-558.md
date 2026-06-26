# String affix conformance closeout (#558)

Status: #558 is a conformance/inventory hardening PR for
`nose.protocols.string_affix_predicates`. It does not change runtime admission
routes or widen exact matching.

## Scope

The PR expands descriptor-visible conformance refs for receiver-method string
prefix/suffix predicates:

- positives for Python, Java, Rust, Swift, JavaScript, and TypeScript prefix and
  suffix methods already routed through the string-affix protocol pack;
- hard negatives for non-string receiver proof, wrong producer provenance, and
  unsupported JavaScript offset argument forms;
- no movement for Go `strings.HasPrefix`/`strings.HasSuffix`, which stays in
  `nose.go.stdlib.namespace_calls` for #549.

## Product Comparison

Baseline ref: `origin/main@def272b356e2062e0831ef88c62716adce93f158`.
Measured current build ref: `66120cd530f46602b4249d181fa659bcc170d1dd`.

Binary hashes:

- baseline: `ae02d6525140aa309eef43553b07ea5f995f43db9df0468f9b6ad43f684fc0db`
- current: `b7f7bd1838df7a32ba700584e95516e561b7d1df38f66b97b7e82bbf868d14d4`

The focused corpus can be recreated in any scratch directory with these files:

`prefix.ts`

```ts
export function hasPrefix(value: string): boolean {
  return value.startsWith("pre")
}
```

`prefix.js`

```js
function hasPrefix(value) {
  return value.startsWith("pre");
}
```

`Prefix.java`

```java
public class Prefix {
  boolean hasPrefix(String value) {
    return value.startsWith("pre");
  }
}
```

`prefix.rs`

```rust
pub fn has_prefix(value: &str) -> bool {
    value.starts_with("pre")
}
```

`prefix.swift`

```swift
func hasPrefix(_ value: String) -> Bool {
    return value.hasPrefix("pre")
}
```

`prefix.py`

```python
def has_prefix(value: str) -> bool:
    return value.startswith("pre")
```

`suffix.py`

```python
def has_suffix(value: str) -> bool:
    return value.endswith("pre")
```

`different_affix.py`

```python
def has_other_prefix(value: str) -> bool:
    return value.startswith("alt")
```

`different_receiver.py`

```python
def has_prefix_other_receiver(value: str, other: str) -> bool:
    return other.startswith("pre")
```

Comparison command:

```sh
nose query <focused-string-affix-corpus> all top=0 --mode semantic --format json
```

Result:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| family count | 1 | 1 |
| families equal | true | true |
| semantic pack count | 49 | 49 |
| string-affix positive fixtures | 2 | 12 |
| string-affix hard negatives | 4 | 7 |
| investigation triggers | 0 | 0 |

Drift classification: intentional conformance/inventory metadata drift only.
Focused query families are unchanged.

## Inventory Comparison

Command:

```sh
nose semantic-pack inventory --format json
```

| Metric | Baseline | Current |
| --- | ---: | ---: |
| packs | 49 | 49 |
| builtin packs | 49 | 49 |
| exact-capable packs | 39 | 39 |
| packs needing coverage | 0 | 0 |
| positive fixtures | 178 | 188 |
| hard negatives | 143 | 146 |
| conformance refs | 321 | 334 |
| unsupported refs | 19 | 20 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
7.196, 9.225, 8.758, 7.349, 9.585, 7.526, 9.653, 26.09, 12.987
```

Current times in milliseconds:

```text
7.291, 7.308, 8.863, 8.954, 8.802, 9.379, 12.909, 16.659, 10.125
```

Median: `9.225 ms -> 8.954 ms` (`-0.271 ms`).

## Rollback

Revert the #558 PR. No runtime admission route, pack id, producer id, or
contract id change is required because the PR only changes conformance metadata,
tests, and documentation.
