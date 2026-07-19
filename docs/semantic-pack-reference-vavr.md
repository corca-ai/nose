# Vavr `List.of` reference semantic pack

Status: shipped local reference pack, `priced-ready`, explicit opt-in, disabled
by default.

## What it adds

The reference pack teaches the existing Java collection-factory kernel about a
small non-builtin surface from `io.vavr:vavr` 0.9.x:

| row | channel | supported calls |
| --- | --- | --- |
| `java.vavr.list.of-five-exact` | external-claim exact | imported `io.vavr.collection.List.of` with exactly five arguments |
| `java.vavr.list.of-three-four-near` | near | the same imported factory with three or four arguments |

No Vavr API is enabled globally. The manifest, Maven evidence, source fixtures,
kernel receipt, selected rows, and channels are content-pinned by the checked-in
[project lock](examples/vavr-list-project-lock-v1.json). The exact result is an
external provider claim tested by nose and authorized by the user; it is not
builtin certification.

## Try it

Run a query with the pack:

```sh
nose query src all top=0 --mode near \
  --semantic-pack-lock docs/examples/vavr-list-project-lock-v1.json
```

Copy the manifest, receipt, dependency evidence, fixtures, and lock together
when using the example outside this repository. Regenerate the receipt and lock
with the installed `nose` after any intentional content or version change.

## Evidence and measured value

Candidate pricing used the pinned Maven consumer
`dhinojosa/vavr-study@10c8b1c649c672e69a75c60669f442f97b8a555e`.
Its `pom.xml` declares `io.vavr:vavr` 0.9.1 directly and its Java tests contain
explicit `io.vavr.collection.List` imports plus arity-three, four, and five
`List.of` calls. The repository is not vendored or executed.

The product evaluation analyzes that pinned source together with checked-in
same-binary comparison controls. With no lock, the controlled semantic result
is absent. With the lock, one attributed external-claim exact family appears;
near adds one family with two attributed occurrences. Removing the lock restores
the original output. The closeout artifact records hashes, counts, negative
matrices, verification, and official-v0.19 runtime comparisons; see the
[#869 machine-readable evidence](../bench/semantic_pack/issue-869-vavr-reference-pack-closeout-2026-07-19.v1.json) for the immutable values.

## Closed boundaries

The pack does not cover:

- Vavr 1.x, transitive or unresolved dependencies, wildcard imports, fully
  qualified calls, static imports, aliases, local shadows, or rebound names;
- arities outside the selected rows, `ofAll`, ranges, streams, maps, sets,
  tuples, options, functional interfaces, lazy operations, or callbacks;
- arbitrary provider matchers, executable provider code, Maven/network access,
  new value-graph nodes, or new canonicalization algorithms.

Wrong dependency versions, imports, members, arities, shadows, receipt content,
fixture content, conflicts, and resource limits fail closed.

## Disable and roll back

Remove `--semantic-pack-lock` or the `semantic-pack-lock` configuration entry.
The manifest may remain present for metadata inspection but cannot influence
analysis without its valid lock. To narrow authority, regenerate the lock with
only `near` or only selected rows. Do not hand-edit digests.

See [semantic-pack project locks](semantic-pack-project-lock.md) for authority,
[conformance](semantic-pack-conformance.md) for receipts, and
[extension API v1](semantic-pack-extension-api-v1.md) for the closed grammar.
