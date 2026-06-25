# Semantic pack conformance

Status: nose provides a local semantic-pack v0 conformance harness for manifest
structure, declared fixture assets, and declarative fixture-expectation gates for
exact-capable rows. The harness is a provider/user workflow, not nose approval of
third-party semantic correctness. External packs remain `metadata-only` when
loaded by `nose query`.

## Goal

The conformance workflow gives pack providers a reproducible way to show that a
pack meets the extension contract's minimum structural obligations:

- the manifest parses as semantic-pack v0;
- stable ids, enum values, trust/default policy, provenance, and compatibility
  declarations are present;
- evidence producers, contracts, laws, dependencies, and conformance references
  are internally linked;
- contracts and value laws declare object-shaped semantics, and exact-capable
  declarations add evidence requirements plus demand/effect semantics;
- fixed call result-domain declarations use known domain vocabulary and require
  `LibraryApi.Contract` occurrence evidence;
- positive fixtures and hard negatives are declared with expectation labels;
- fixture files exist at paths relative to the manifest file;
- optional executable gates bind exact-capable producer, contract, and value-law
  rows to declared positive and hard-negative fixture expectations.

This keeps the ecosystem boundary narrow: packs publish evidence, contracts, and
fixtures; the kernel decides whether evidence is admissible for a channel; users
decide whether to enable external packs.

## Non-goals

The harness does not:

- execute external evidence producers;
- register external contract rows with exact consumers;
- treat a passing structural check as permission for external rows to influence
  analysis;
- execute fixture contents, provider commands, recognizers, parser/lowering
  plugins, producer code, or sandboxed code as an oracle;
- prove that the provider's semantic claims are complete or true;
- certify, approve, rank, or endorse external packs.

Builtin default packs are different. nose owns their tests, hard negatives,
proof obligations, release gates, and documentation because those packs ship with
the binary and affect exact analysis by default. The Python stdlib type-domain
example manifest mirrors the first compiled builtin pilot pack, but local
copies of that manifest still load only as external metadata unless nose ships
the pack as compiled builtin code.
Compatibility policy and installed-version range checks are documented in
[semantic-pack-compatibility](semantic-pack-compatibility.md).

## Command

Run the harness against a manifest file or a directory of direct `*.json`
manifests:

```sh
nose semantic-pack check semantic-packs/python-math-prod.json
nose semantic-pack check docs/examples/semantic-packs/v0 --format json
```

Directory checking follows the same local discovery rule as pack loading: direct
JSON children only, sorted by filename, no recursion, no registry, and no network.
Relative fixture paths are resolved from the manifest's directory.

The command exits non-zero when any manifest is invalid, when pack ids collide
with each other or any compiled builtin pack id, when declared conformance
fixtures are missing a path, missing an expectation, or point at a file that does
not exist, or when a declared executable gate does not match its fixture
expectation oracle. For `--format json`, the command still writes the report
before returning the non-zero exit after manifests have loaded. Manifest parse,
schema, compatibility, reserved-id, or duplicate-id errors that prevent report
creation are returned as load errors instead.
The example [law pack](examples/semantic-packs/v0/law-pack.json) uses this
workflow to declare value-law positives and hard negatives. Passing the check
confirms only that the law metadata and fixture assets are structurally present;
it does not register those external laws with exact consumers.
Its expectation labels distinguish report-visible positives such as
`semantic-law-provenance-present` from narrower unit-level fixtures such as
`internal-law-unit-positive`; the harness preserves those labels but does not
execute them.
The R4 [Guava immutable collection example](examples/semantic-packs/v0/external-guava-immutable-collections-pack.json)
uses the same workflow for a fixed result-domain contract. It demonstrates that
external providers can author `semantics.result_domain`, fixtures, hard
negatives, and a fixture-expectation gate while the influence preflight still
blocks exact use of the row.

Builtin packs use a separate inventory command because they are compiled into
nose and already influence analysis:

```sh
nose semantic-pack inventory
nose semantic-pack inventory --format json
```

The inventory is static and cheap: it reads compiled builtin descriptors, not
source files or external manifests. It reports each builtin pack's declaration
ids, conformance refs, positive and hard-negative refs, unsupported refs,
pack-level exact-capable coverage status, and audit gaps. Exact-capable means
the pack has descriptor-declared exact rows, or conformance-backed producers
whose output already influences exact consumers. Exact-capable builtin packs
with missing positive or hard-negative refs report `needs-coverage`; packs with
no exact-capable rows report `tracked-no-exact-rows`.

The coverage status is a descriptor-level audit. It verifies that a builtin pack
has both positive and hard-negative conformance references, and it reports gaps
when fixture counts and classified refs diverge. It does not yet prove that each
individual producer, contract, law, or alias row has a one-to-one fixture pair;
that narrower row-level coverage belongs in a later schema version.

The inventory also records that product-output and performance evidence are
`required-on-implementation-pr`. Those measurements live in the PR that changes
behavior or pack ownership, not in a static descriptor table.

## Builtin Inventory JSON

`nose semantic-pack inventory --format json` emits schema version 1:

```json
{
  "schema_version": 1,
  "status": "ok",
  "totals": {
    "packs": 46,
    "builtin_packs": 46,
    "exact_capable_packs": 36,
    "packs_needing_coverage": 0,
    "positive_fixtures": 150,
    "hard_negatives": 102,
    "conformance_refs": 252,
    "unsupported_refs": 13
  },
  "evidence_policy": {
    "product_output": "required-on-implementation-pr",
    "performance": "required-on-implementation-pr"
  },
  "packs": [
    {
      "id": "nose.go.stdlib.namespace_calls",
      "kind": "StdlibPack",
      "declarations": {
        "contracts": ["go.stdlib.namespace_call"],
        "type_domain_aliases": [],
        "counts": {
          "contracts": 1,
          "positive_fixtures": 5,
          "hard_negatives": 2
        }
      },
      "conformance": {
        "positive_refs": [
          "go-stdlib-namespace-call-fmt-print-positive",
          "go-stdlib-namespace-call-strings-has-prefix-positive",
          "go-stdlib-namespace-call-strings-has-suffix-positive",
          "go-stdlib-namespace-call-slices-contains-positive",
          "go-stdlib-namespace-call-strings-contains-positive"
        ],
        "hard_negative_refs": [
          "go-stdlib-namespace-call-missing-import-hard-negative",
          "go-stdlib-namespace-call-wrong-pack-hard-negative"
        ],
        "unsupported_refs": []
      },
      "audit": {
        "exact_capable": true,
        "coverage_status": "covered",
        "gaps": []
      }
    }
  ]
}
```

Important fields:

| Field | Meaning |
| --- | --- |
| `totals.exact_capable_packs` | Builtin packs whose declarations or conformance-backed producers currently influence exact semantic consumers. |
| `packs[].declarations.type_domain_aliases` | Type-domain alias coordinates, formatted as `<contract-id>:<module>.<exported>:<domain>`. |
| `packs[].conformance.positive_refs` | Conformance refs classified as positive fixtures by their stable fixture id. |
| `packs[].conformance.hard_negative_refs` | Conformance refs classified as hard negatives by their stable fixture id. |
| `packs[].conformance.unsupported_refs` | Refs whose stable id explicitly contains `unsupported`; other hard negatives remain in `hard_negative_refs`. |
| `packs[].audit.coverage_status` | One of `covered`, `needs-coverage`, or `tracked-no-exact-rows`. |
| `packs[].audit.gaps` | Descriptor-level audit gaps such as missing positive refs, missing hard negatives, unclassified ref polarity, or fixture count mismatch. |

## Check JSON Report

`--format json` emits schema version 2:

```json
{
  "schema_version": 2,
  "status": "ok",
  "totals": {
    "manifests": 1,
    "positive_fixtures": 1,
    "hard_negatives": 2,
    "fixture_issues": 0,
    "executable_conformance_rows": 1,
    "passed_executable_conformance_rows": 1,
    "executable_conformance_issues": 0,
    "influence_rows": 1,
    "blocked_influence_rows": 1
  },
  "executable_conformance": {
    "status": "ok",
    "rows": [
      {
        "gate_id": "python.example.contract.gate",
        "kind": "contract",
        "row_id": "python.example.contract",
        "row_hash": "0123456789abcdef",
        "pack_id": "com.example.semantic-pack",
        "pack_hash": "fedcba9876543210",
        "manifest_path": "/repo/packs/example.json",
        "channel": "exact-empirical",
        "oracle": "fixture-expectations",
        "passed": true,
        "positive_fixtures": ["positive"],
        "hard_negatives": ["negative"],
        "issues": []
      }
    ]
  },
  "influence_preflight": {
    "status": "blocked",
    "rows": [
      {
        "kind": "contract",
        "row_id": "python.example.contract",
        "row_hash": "0123456789abcdef",
        "pack_id": "com.example.semantic-pack",
        "pack_hash": "fedcba9876543210",
        "manifest_path": "/repo/packs/example.json",
        "channel": "exact-empirical",
        "passed": false,
        "blockers": [
          "data-only-registration",
          "dependency-backed-evidence-unavailable",
          "explicit-influence-trust-gate-missing"
        ]
      }
    ]
  },
  "manifests": [
    {
      "id": "com.example.semantic-pack",
      "version": "0.1.0",
      "display_name": "Example semantic pack",
      "trust": "external-opt-in",
      "source": "local-manifest",
      "influence": "metadata-only",
      "manifest_path": "/repo/packs/example.json",
      "provider": "Example Semantic Packs",
      "repository": "https://example.invalid/semantic-packs/example",
      "license": "MIT",
      "supported_languages": ["python"],
      "counts": {
        "evidence_producers": 1,
        "contracts": 1,
        "value_laws": 0,
        "positive_fixtures": 1,
        "hard_negatives": 2
      },
      "conformance_command": "nose semantic-pack check example.json --format json",
      "proof_links": [],
      "fixture_issues": 0,
      "fixtures": [
        {
          "kind": "positive",
          "id": "positive",
          "description": "Fixture expected to satisfy the declared row.",
          "declared_path": "fixtures/positive.py",
          "resolved_path": "/repo/packs/fixtures/positive.py",
          "expectation": "contract-present",
          "issues": []
        },
        {
          "kind": "hard-negative",
          "id": "negative",
          "description": "Fixture expected not to satisfy the declared row.",
          "declared_path": "fixtures/negative.py",
          "resolved_path": "/repo/packs/fixtures/negative.py",
          "expectation": "contract-absent",
          "issues": []
        },
        {
          "kind": "hard-negative",
          "id": "shadowed-negative",
          "description": "Fixture expected to remain closed under shadowing.",
          "declared_path": "fixtures/shadowed_negative.py",
          "resolved_path": "/repo/packs/fixtures/shadowed_negative.py",
          "expectation": "symbol-proof-closed",
          "issues": []
        }
      ]
    }
  ]
}
```

Each manifest entry includes pack provenance, declaration counts, the optional
provider-supplied conformance command, proof links, and per-fixture issue labels
such as `missing-path`, `missing-expectation`, `missing-file`, and
`absolute-path`.

The top-level `executable_conformance` object is the machine-readable proof that
declared exact-capable rows passed the local fixture-expectation gate. Its status
is `unavailable` when no gates are declared, `ok` when all declared gates pass,
and `failed` when any declared gate reports issues such as `unknown-fixture`,
`wrong-fixture-kind`, `missing-expectation`, `fixture-issue`, or
`expectation-mismatch`.

The JSON report also includes `influence_preflight`. This is a row-level
admission preview for local external declarations, not a grant of influence.
Exact-capable rows without a passed executable gate report
`executable-conformance-unavailable`. Rows with a passed gate clear only that
blocker; v0 external rows still report blockers such as
`data-only-registration`, `dependency-backed-evidence-unavailable`,
`explicit-influence-trust-gate-missing`, and `row-conflict`. The `totals` object
includes `influence_rows` and `blocked_influence_rows` so integrations can fail a
provider workflow when a pack is structurally valid but still not admissible for
analysis. Row and pack hashes are stable 16-hex-digit strings.

Integrations should discover support through [capabilities](capabilities.md):
`commands.stable` includes `semantic-pack`, `schemas.semantic_pack_conformance`
and `schemas.semantic_pack_inventory` list supported report schema versions, and
`semantic_packs.conformance` / `semantic_packs.inventory` list accepted input
sources.

## Provider Responsibilities

External pack providers own:

- the truth of every language, runtime, package, API, protocol, law, demand,
  effect, mutation, exception, and version claim;
- fixture quality and hard-negative coverage;
- proof quality for any `exact-proven` claim;
- provenance, license, repository, support contact, and release notes;
- the decision to publish updates or deprecate unsupported claims.

Passing `nose semantic-pack check` means the pack is structurally well-formed and
its declared fixtures are present. It does not mean the pack is safe to enable in
every user's risk model, and it does not let external rows influence query,
normalize, value-graph, exact, or detection consumers.

## User Responsibilities

Users who opt into external packs own the enablement decision. They should inspect:

- provider identity and repository history;
- package/version ranges and unsupported boundaries;
- positive and hard-negative fixtures;
- executable fixture-expectation gates for exact-capable rows, when present;
- whether exact-capable contracts are appropriate for their codebase;
- the pack's own test or proof evidence outside nose.

`nose query --semantic-pack` and `[query].semantic-packs` validate local
external pack manifests and query JSON schema v7 reports the active pack set in
top-level `semantic_packs`. Future producer execution must keep the same
provenance and fail-closed behavior before external packs can affect `near` or
exact results.

## See also

- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md)
- [semantic-pack-loading](semantic-pack-loading.md)
- [semantic-kernel](semantic-kernel.md)
