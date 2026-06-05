# Languages

nose parses each language with tree-sitter and lowers it into one shared IL, so
clones are found *across* languages, not just within one. Back to [home](home.md); the
lowering machinery is described in [architecture](architecture.md).

## Supported languages

Eight base languages, each with its own CST→IL lowering:

| language | extensions |
|---|---|
| Python | `.py`, `.pyi` |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` |
| TypeScript | `.ts`, `.tsx`, `.mts`, `.cts` |
| Go | `.go` |
| Rust | `.rs` |
| Java | `.java` |
| C | `.c`, `.h` |
| Ruby | `.rb` |

JSX and TSX are handled by the JavaScript/TypeScript lowering path (the type
syntax is erased during [normalization](normalization.md)).

## Embedded `<script>` in Vue / Svelte / HTML

`.vue`, `.svelte`, and `.html`/`.htm` files carry their logic inside `<script>`
blocks. nose extracts those regions and lowers them as JS/TS, so duplication
between a component's script and a plain `.ts` file is found like any other
clone. Extraction **blanks** the non-script bytes (replacing them with spaces
while keeping newlines), so reported line numbers point at the exact lines in the
original `.vue`/`.svelte`/`.html` file.

This is why a helper duplicated across a Svelte component and a TypeScript module
shows up as one cross-container family (confirmed on real projects in
[field-evaluation](field-evaluation.md)).

## Coverage and adding a language

Lowering quality is measured by the **Raw-node ratio** — the fraction of CST
nodes that fall through to an opaque `Raw` IL node instead of a real one. Lower
is better; the vendored corpus sits below 0.01%. Check it per language with:

```sh
nose stats <paths…>
```

A high Raw ratio for a construct means that construct isn't lowering to a
meaningful IL shape, so clones involving it won't converge. Closing those gaps
(e.g. the Go composite-literal/`slice_expression`/`type_assertion` work that took
Go from 0.40% to 0.03%, or Java local record/annotation declarations being erased
as type metadata instead of surfacing as opaque statements, or Rust `async { ... }`
blocks lowering to their body instead of a `Raw` wrapper, or Rust
literal/negative-literal/typed-integer/wildcard/range/tuple/slice/reference/OR/guarded
`match` arms lowering to an if-chain, or Python
literal/wildcard/capture/qualified/sequence/OR/as/guarded `match` cases lowering to an if-chain, or Go
channel send statements lowering to a tagged effect shape, or Go multi-label `switch` cases lowering to
ORed scrutinee comparisons, or JS/TS stacked `switch` case labels sharing the following
body, or C/Java `switch` labels lowering to real scrutinee comparisons instead of
placeholder branches, or Java `switch` expression rules, including block `yield` bodies,
lowering to expression if-chains instead of `Raw`, or Ruby scrutinee-less `case` lowering
its `when` predicates directly while preserving the `else` arm) is how a language becomes a
first-class citizen —
see the [experiments](experiments.md) log and the convergence-test discipline in
[`CONTRIBUTING`](../CONTRIBUTING.md).
