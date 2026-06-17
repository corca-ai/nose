# Languages

nose parses each language with tree-sitter and lowers it into one shared IL, so
clones are found *across* languages, not just within one. The lowering machinery is
described in [architecture](architecture.md).

The current language frontends are first-party code. The direction is for both
first-party and external languages to enter through the same pack extension
boundary described in [semantic-kernel](semantic-kernel.md), while keeping exact
semantic matching fail-closed unless the required facts and contracts are present.

## Supported languages

Eight **imperative** base languages, each with its own CST→IL lowering:

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

JSX and TSX are handled by the JavaScript/TypeScript lowering path (the type syntax is
erased during [normalization](normalization.md)). Their **JSX markup** is lowered into
the shared declarative markup IL (below), not an imperative call tree — so a React
component's markup is clone-matched against HTML/Vue/Svelte markup (see
[cross-dialect markup](#cross-dialect-markup-htmlvuesveltejsxtsx)).

## Declarative languages: CSS

| language | extensions |
|---|---|
| CSS | `.css` |

CSS is **declarative**: a rule's meaning is its *computed style*, not imperative
execution. So CSS does not ride the imperative value graph (GVN). Instead each CSS
**rule** is a detection unit whose exact `semantic` fingerprint is the **canonical
computed/declared style** of its declaration block — see
[normalization › declarative (CSS) fingerprint](normalization.md). Two rules are an
exact clone when they compute the same style, so the same duplicated declaration block
under different selectors is one family. Concretely the fingerprint is invariant to:

- **selector** — `.btn { … }` and `.cta { … }` with the same declarations merge (a
  duplicated declaration block is the canonical CSS clone);
- **declaration order** — except where it changes the cascade (see below);
- **value spelling** — `#fff` ≡ `#ffffff` ≡ `white` ≡ `rgb(255 255 255)`; `0px` ≡ `0`;
  `margin: 0 0 0 0` ≡ `margin: 0`; trailing-zero/sign noise.

It is deliberately kept apart (no false merge) by:

- **cascade** — a repeated property keeps the last (`color:red; color:blue` ≢ reverse),
  and a shorthand mixed with one of its longhands cascades by order
  (`margin:0; margin-top:5px` ≢ reverse);
- **at-rule context** — a `@media`-scoped rule never merges with an unconditional one;
- **domain disjointness** — a CSS fingerprint can never equal an imperative one, so the
  (language-blind) exact channel cannot merge CSS with code.

Soundness for CSS is **by construction** (the fingerprint *is* the canonical computed
style) plus adversarial per-rule batteries (the project's primary trust mechanism — see
[design](design.md)); the value normalizations live in `nose-normalize::css_value`, each
with positive and hard-negative tests. A standalone interpreter oracle (as for the
imperative languages) is redundant for a declarative domain where the fingerprint is the
denotation. Lowering coverage is first-class: the [Raw-node ratio](#coverage-and-adding-a-language)
on real-world `.css` is ~0.2% (the residue is non-standard PostCSS at-statements, left
as honest `Raw`).

## Declarative languages: HTML markup

| language | extensions |
|---|---|
| HTML | `.html`, `.htm` (markup) |

HTML markup is also **declarative**: an element's meaning is the **rendered DOM** it
produces. Each `HtmlElement` subtree is a detection unit whose exact `semantic`
fingerprint is the canonical DOM of that subtree (`nose-normalize::html`), so two
markup blocks are an exact clone when they render the same DOM. The fingerprint
normalizes the DOM-insignificant: **attribute order**, **boolean-attribute form**
(`disabled` ≡ `disabled=""`), **`class` token order** (a set), tag/attribute-name case,
and insignificant whitespace. It keeps **tag/structure**, **child order**, **text**, and
**attribute values** distinct, so a content difference never merges. As with CSS, the
structural `near` channel additionally scores **structure-only** similarity (text and
volatile values abstracted), which is what surfaces "the same repeated component shell
with different content" — the highest-value markup clone. Soundness is by construction
plus adversarial batteries; HTML and CSS fingerprints are domain-disjoint from each other
and from imperative code. Real-world markup lowers at a first-class Raw-node ratio (~0.4%;
the rare residue is malformed/generated pages, left as honest `Raw`).

## Embedded `<script>` / `<style>` in HTML / Vue / Svelte

`.html`/`.htm`, `.vue`, and `.svelte` files mix logic, style, and markup. nose lowers
each file into **several regions**, analyzed independently and all sharing the file's
path: `<script>` blocks as JS/TS, `<style>` blocks as CSS, and the markup tree as HTML
(a `.vue`/`.svelte` `<template>` parses as markup too). Region extraction **blanks** the
other bytes (replacing them with spaces while keeping newlines), so reported line numbers
point at the exact lines in the original file; `<script>`/`<style>` *internals* are not
double-counted in the markup tree. Preprocessor `<style lang="scss"|"less"|…>` blocks are
skipped (out of scope).

So a helper duplicated across a Svelte component and a TypeScript module — or a
declaration block shared between a component's `<style>` and a plain `.css` file, or a
repeated card across two HTML pages — shows up as one cross-container family (script
cross-container confirmed on real projects in [field-evaluation](field-evaluation.md)).

## Cross-dialect markup (HTML/Vue/Svelte/JSX/TSX)

The five markup dialects render to the same thing — a DOM tree — so nose lowers them all
into the **one** declarative markup IL and clone-matches across them. The same component
written in React, Vue, and Svelte converges. This works because each dialect's frontend
normalizes its idioms into the shared IL:

- **control flow** → an `HtmlControl` wrapper: Vue `v-for`/`v-if`, Svelte
  `{#each}`/`{#if}`, and JSX `.map`/`&&`/ternary all become a `repeat`/`if` control. The
  wrapper is structurally distinct from a plain element, so a loop never exact-merges with
  a single element (sound), while the `near` channel abstracts it so the three control
  idioms converge;
- **directives** are classified: events/lifecycle/bookkeeping (`@click`/`v-on:`, `on:`,
  `use:`, `key`, `ref`) are dropped (not rendered); a bound real attribute
  (`:src`/`v-bind:`, Svelte `bind:value`, `v-model`) keeps the rendered attribute name
  with its **expression as the value, verbatim** — so two different bound expressions stay
  distinct on the exact channel;
- **attribute/tag aliases** that render the same DOM are unified: JSX `className`→`class`,
  `htmlFor`→`for`; routing components/props `router-link`/`<Link>`→`a`, `to`→`href`;
- **interpolation** (`{x}`, `{{ x }}`) keeps its verbatim text on the exact channel (so
  different expressions never merge) and is abstracted by `node_tag` on the `near` channel
  (so same-shell-different-content components converge — the headline cross-dialect clone);
- `<script>`/`<style>` elements are dropped from the markup tree (analyzed as their own
  regions, above), and inline `style="…"` is computed-canonicalized via the CSS path.

Soundness is preserved end-to-end: a cross-dialect match lands on the exact channel only
when the rendered DOM is genuinely identical (e.g. a static nav link), otherwise on the
structural `near` channel — never a false claim of behavioral equality. See the adversarial
battery in `crates/nose-cli/tests/equivalence.rs` (`markup_*`).

Out of scope (see [clone-types](clone-types.md)): SCSS/Less/Sass and CSS `var()`
resolution across files. Component composition that differs across dialects (one dialect
extracts a sub-component where another inlines it) is matched at the shared-subtree level,
not whole-component, which is correct.

## Coverage and adding a language

Lowering quality is measured by the **Raw-node ratio** — the fraction of CST
nodes that fall through to an opaque `Raw` IL node instead of a real one. Lower
is better. `nose stats` distinguishes two kinds of Raw: by-design
**protocol-boundary** Raw (await, try/`?`, defer, go, channel operations, select,
yield — fail-closed boundaries, not coverage gaps) from genuine **lowering-gap**
Raw. It reports `boundary_raw` and tags each unhandled construct `boundary` or
`gap`. On the current pinned `bench/repos` corpus the overall ratio is in the low
single-digit percent; run `nose stats` for the current figure, with
language-specific gaps visible per construct. Check it per language with:

```sh
nose stats <paths…>
```

A high *gap* ratio for a construct (not a by-design boundary) means that construct
isn't lowering to a meaningful IL shape, so clones involving it won't converge. Closing those gaps
(e.g. the Go composite-literal/`slice_expression`/`type_assertion` work that took
Go from 0.40% to 0.03%, or Java local record/annotation declarations being erased
as type metadata instead of surfacing as opaque statements, or Rust `async { ... }`
blocks lowering to their body instead of a `Raw` wrapper, or Rust
literal/negative-literal/typed-integer/wildcard/range/tuple/slice/reference/OR/guarded
`match` arms lowering to an if-chain, or Python
literal/wildcard/capture/qualified/sequence/OR/as/guarded `match` cases lowering to an if-chain, or Go
channel operations lowering to source-backed protocol boundaries, or Go multi-label `switch` cases lowering to
ORed scrutinee comparisons, or JS/TS stacked `switch` case labels sharing the following
body, or C/Java `switch` labels lowering to real scrutinee comparisons instead of
placeholder branches, or Java `switch` expression rules, including block `yield` bodies,
lowering to expression if-chains instead of `Raw`, or Ruby scrutinee-less `case` lowering
its `when` predicates directly while preserving the `else` arm) is how a language becomes a
first-class citizen —
see the [experiments](experiments.md) log and the convergence-test discipline in
[`CONTRIBUTING`](../CONTRIBUTING.md).

For the planned pack-based language onboarding model, see
[semantic-kernel-roadmap](semantic-kernel-roadmap.md).
