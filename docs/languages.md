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
Go from 0.40% to 0.03%) is how a language becomes a first-class citizen — see the
[experiments](experiments.md) log and the convergence-test discipline in
[`CONTRIBUTING`](../CONTRIBUTING.md).
