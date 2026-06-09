# Semantic pack loading

Back to [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md),
[semantic-pack-conformance](semantic-pack-conformance.md), and
[semantic-kernel](semantic-kernel.md).

Status: nose can load local semantic-pack v0 manifests for provenance and trust
reporting, and it can run a separate local conformance check for manifests and
declared fixture assets. External packs are explicit opt-ins and are currently
`metadata-only`: they do not emit evidence, open exact contracts, mint
fingerprints, or approve clone pairs.

## Local entry points

Use `--semantic-pack <file-or-dir>` on `nose scan` to opt into local pack
metadata for one run:

```sh
nose scan src --format json --semantic-pack semantic-packs/python-math-prod.json
```

Commit stable project opt-ins in `nose.toml`:

```toml
[scan]
semantic-packs = ["semantic-packs/python-math-prod.json"]
```

Each path may be a manifest file or a directory. Paths from `[scan].semantic-packs`
are resolved relative to the config file that declared them; paths from
`--semantic-pack` are resolved by the shell/current working directory like other
CLI paths. Directory loading reads direct `*.json` children in sorted order; it
does not recurse and it does not contact a registry or network service.

## Conformance entry point

Pack authors and users can check the same local manifest paths without loading
them into a scan:

```sh
nose semantic-pack check semantic-packs/python-math-prod.json
nose semantic-pack check semantic-packs --format json
```

The conformance command validates manifest structure, trust policy, dependency
references, exact-capable contract obligations, conformance fixture references,
fixture expectation labels, and fixture file existence. It does not execute
external producers or certify semantic correctness. See
[semantic-pack-conformance](semantic-pack-conformance.md).

## Trust policy

Trust is separate from channel eligibility.

- The compiled first-party pack `nose.first_party` is enabled by default and is
  the only pack that currently influences evidence and contracts.
- Local external packs require explicit user opt-in through CLI or config.
- Local manifests must declare `trust = "external-opt-in"` and
  `enabled_by_default = false`; manifests that claim first-party trust or default
  enablement are rejected.
- Duplicate pack ids fail the scan instead of letting provenance become
  ambiguous.

`scan --format json` reports every active pack under `semantic_packs`, including
its stable hash, trust, source, influence, provider, repository, supported
languages, and declaration counts. Human and Markdown reports print a short
semantic-pack line when local opt-in packs are loaded.

## Current limits

The loader validates manifest shape and pack provenance only. It does not yet:

- execute external evidence producers;
- register external contract rows with exact consumers;
- execute fixture contents or run a behavioral oracle;
- compare semantic version ranges against the installed nose version beyond
  requiring a parseable declared compatibility field;
- install packs from a registry or remote source.

Future loader work should keep this boundary: external pack claims can become
usable only through dependency-backed evidence records and fail-closed kernel
contracts, never through raw selectors or manifest presence alone.
