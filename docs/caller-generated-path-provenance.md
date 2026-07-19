# Caller-provided generated-path provenance

Issue [#925](https://github.com/corca-ai/nose/issues/925) defines the smallest
caller-supplied provenance contract that can classify checked-in generated artifacts without
deleting their findings. It complements nose's source-derived generated-file rules; it does
not add repository, producer, language, symbol, or filename allowlists to the detector.

## Contract

Local users can repeat `--generated-path <GLOB>`. Projects can commit the same assertions in
`nose.toml` or `.nose.toml`:

```toml
[query]
generated-paths = ["generated/**", "**/snapshots/mypy/**"]
```

Command-line and config patterns are additive. Duplicate patterns collapse before matching.
`--config` selects the config file by the existing config rules; patterns remain relative to
the analyzed roots, not to the config file, so a committed contract is portable across
checkout locations.

Patterns use gitignore glob syntax but are positive, root-relative assertions. nose anchors
each pattern automatically to every explicit analysis root:

- `generated/**` matches only a `generated` directory immediately below a root;
- `**/generated/**` also matches nested `generated` directories;
- a file root is matched by its file name;
- a path is marked when it matches relative to any containing root, so overlapping and
  multi-root queries have deterministic union semantics.

Empty patterns, negation (`!`), absolute paths, `.` or `..` path components, and backslashes
are rejected before analysis. This keeps one portable spelling and prevents a pattern from
escaping or changing meaning with the current directory.

## Trust, containment, and failure

A generated-path entry is a caller assertion, not a nose inference and not proof that a
producer ran. nose canonicalizes both roots and candidate files before matching. A symlink
supplied as an explicit root therefore describes the tree it resolves to, while a symlink
inside a root that escapes that tree cannot acquire generated provenance. Files that are
missing, unreadable, not regular files, or outside every canonical root fail open.

The assertion changes only presentation. A family receives `surface: "generated"` when
every member has either caller-supplied or nose-inferred generated provenance. Partial and
mixed families retain their prior surface. The existing compiled-CSS source/output rule
remains nose-inferred and unchanged. No assertion changes discovery, lowering, candidates,
family construction, family IDs, ordering, witnesses, ranking, helper selection, baseline
identity, or existing non-surface fields.

Generated families remain recoverable with `all top=0` or `surface=generated`. The human
default continues to explain their omission as `generated-code`; Markdown, SARIF, and
`--fail-on` use the same effective surface instead of deleting the family. Structured
ignores and `--exclude` remain separate contracts: ignores suppress accepted findings with
an audit record, while excludes prune inputs before analysis.

## Machine contract

`nose capabilities` advertises the `caller_generated_paths` query capability and the
`generated-paths` config key. Integrations should preflight that capability before passing
`--generated-path`.

Every generated family in non-`base` query JSON has additive provenance:

```json
"generated_provenance": {
  "basis": "all-members",
  "sources": ["caller-path"]
}
```

`basis` is `all-members` or `compiled-css-pipeline`. `sources` is a sorted, deduplicated list
containing `caller-path`, `nose-inferred`, or both. This is query JSON schema v9: the existing
strict v7 contract is not silently extended, and no v7 field changes meaning or type.
`base=<ref>` is a separate divergent-edit contract and rejects generated-path inputs rather
than silently ignoring them.

## Why no producer manifest yet

The audited Check and Pydantic residuals are already identified by stable project paths, and
the all-member rule still protects mixed maintained/generated families. A producer manifest
would add a new lifecycle, source/output identity model, and schema without improving that
boundary. The path contract is therefore independently sufficient for this slice. A future
manifest should start only from evidence that source/output relationships enable a safer
decision the path contract cannot express; it is not part of #925.
