# Capabilities contract

`nose capabilities` emits the stable machine-readable contract for the installed
binary. Use it from installers, editor integrations, CI wrappers, and doctor
commands before invoking `nose query`. For the human command guide see
[usage](usage.md); for the result JSON see [query-json](query-json.md).

## Why this is not help text

`nose --help` is a human interface. It can change wording, examples, wrapping,
and ordering to improve readability. Tools should not scrape it.

`nose capabilities` is an integration interface. It is JSON-only, has its own
`schema_version`, and reports what the binary supports as data: stable commands,
detection modes, output formats, schema versions, config keys, and capability flags.

Integration rule: branch on `schema_version`, ignore unknown fields, and test capability
flags before passing optional query arguments. A wrapper that does this can run against older
and newer nose binaries without scraping help text or guessing from the package version.

## Example

```sh
nose capabilities
```

```json
{
  "schema_version": 4,
  "tool": {
    "name": "nose",
    "version": "<version>"
  },
  "platform": {
    "os": "linux",
    "arch": "x86_64",
    "family": "unix"
  },
  "interfaces": {
    "capabilities_json": true,
    "version_json": false,
    "doctor_json": false
  },
  "commands": {
    "stable": ["capabilities", "il", "query", "semantic-pack", "stats"],
    "deprecated": []
  },
  "schemas": {
    "capabilities": [4],
    "query_json": [7],
    "semantic_packs": ["nose.semantic-pack.v0"],
    "semantic_pack_conformance": [2],
    "semantic_pack_inventory": [1],
    "semantic_pack_adoption_gates": [1],
    "semantic_pack_compatibility": [1]
  },
  "query": {
    "modes": ["syntax", "semantic", "near"],
    "default_modes": ["syntax", "semantic", "near"],
    "output_formats": ["human", "json", "markdown", "sarif"],
    "sort_keys": ["extractability", "value", "sites", "hazard"],
    "config_keys": [
      "exclude",
      "ignore-file",
      "min-lines",
      "min-members",
      "min-size",
      "min-value",
      "mode",
      "semantic-packs",
      "sort"
    ],
    "capabilities": {
      "base_divergence": true,
      "baseline": true,
      "baseline_changed_detection": true,
      "baseline_member_digest": true,
      "cache": true,
      "ci_fail_gate": true,
      "family_drilldown": true,
      "inline_suppression": true,
      "multi_root": true,
      "reinvented_view": true,
      "semantic_pack_loading": true,
      "structured_ignores": true
    }
  },
  "semantic_packs": {
    "api_versions": ["nose.semantic-pack.v0"],
    "loading": [
      "compiled-builtin",
      "local-manifest-file",
      "local-manifest-directory"
    ],
    "conformance": [
      "local-manifest-file",
      "local-manifest-directory"
    ],
    "conformance_output_formats": ["human", "json"],
    "inventory": ["compiled-builtin"],
    "inventory_output_formats": ["human", "json"],
    "adoption_gates": ["compiled-builtin"],
    "adoption_gate_output_formats": ["human", "json"],
    "compatibility": ["policy"],
    "compatibility_output_formats": ["human", "json"],
    "trust": [
      "builtin-default",
      "builtin-optional",
      "external-opt-in"
    ],
    "external_packs_enabled_by_default": false,
    "external_pack_influence": "metadata-only",
    "external_influence_blockers": [
      "data-only-registration",
      "dependency-backed-evidence-unavailable",
      "explicit-influence-trust-gate-missing",
      "executable-conformance-unavailable",
      "row-conflict"
    ],
    "external_pack_execution": "none"
  },
  "il": {
    "output_formats": ["sexpr", "json"],
    "normalized": true,
    "cfg_norm_toggle": true
  },
  "stats": {
    "output_formats": ["human", "json"]
  }
}
```

`query.modes` lists stable detection modes only. Hidden experimental modes such as
`abstraction` may be accepted by a development binary without appearing here; wrappers
should treat absence from `query.modes` as "not stable for automation."

`tool.version` is shown as the `<version>` placeholder because the field always reports the
installed binary's own version (`nose --version`); the example deliberately does not pin a
release so it can't drift.

## Version 4 Fields

| field | type | meaning |
|---|---|---|
| `schema_version` | integer | Capabilities contract version. Version 4 is documented here. |
| `tool.name` | string | Always `nose`. |
| `tool.version` | string | Package version of the installed binary. |
| `platform.os` | string | Rust target OS name, such as `linux`, `macos`, or `windows`. |
| `platform.arch` | string | Rust target architecture, such as `x86_64` or `aarch64`. |
| `platform.family` | string | Rust target family, such as `unix` or `windows`. |
| `interfaces.capabilities_json` | boolean | Whether `nose capabilities` is the supported capability query interface. |
| `interfaces.version_json` | boolean | Whether `nose --version --json` is supported. Version 1 reports `false`. |
| `interfaces.doctor_json` | boolean | Whether `nose doctor --json` is supported. Version 1 reports `false`. |
| `commands.stable` | array | Stable user-facing commands that integrations may invoke (incl. `query`, the interactive exploration surface — see [usage › nose query](usage.md#nose-query), with its versioned [query-JSON](query-json.md) contract). Hidden research commands are intentionally omitted. |
| `commands.deprecated` | array | Commands that still work but are being retired. Version 4 reports an empty array. |
| `schemas.capabilities` | array | Supported capabilities schema versions. |
| `schemas.query_json` | array | Supported `nose query --format json` schema versions ([query-json](query-json.md)). |
| `schemas.semantic_packs` | array | Supported semantic-pack manifest API versions, currently `nose.semantic-pack.v0`. |
| `schemas.semantic_pack_conformance` | array | Supported `nose semantic-pack check --format json` schema versions. Version 2 reports structural conformance, executable fixture-expectation gate results, and row-level external influence preflight blockers. |
| `schemas.semantic_pack_inventory` | array | Supported `nose semantic-pack inventory --format json` schema versions. Version 1 reports compiled builtin pack declarations, conformance refs, coverage status, and audit gaps. |
| `schemas.semantic_pack_adoption_gates` | array | Supported `nose semantic-pack adoption-gates --format json` schema versions. Version 1 reports compiled builtin pack optional/default promotion gates, rollback actions, and blocker status. |
| `schemas.semantic_pack_compatibility` | array | Supported `nose semantic-pack compatibility --format json` schema versions. Version 1 reports manifest API, installed-version, kernel-vocabulary, and external-influence compatibility policy. |
| `query.modes` | array | Supported `--mode` values. |
| `query.default_modes` | array | Modes used by `nose query` when `--mode` is omitted. |
| `query.output_formats` | array | Supported `nose query --format` values. |
| `query.sort_keys` | array | Supported `sort=` values. |
| `query.config_keys` | array | Supported `[query]` keys in `nose.toml` / `.nose.toml`. |
| `query.capabilities` | object | Stable boolean capability flags for query workflows. |
| `semantic_packs.api_versions` | array | Supported semantic-pack manifest API versions. |
| `semantic_packs.loading` | array | Supported loading sources. Schema version 4 reports `compiled-builtin` for compiled builtin packs, plus local manifest files/directories. |
| `semantic_packs.conformance` | array | Supported conformance input sources: local manifest files/directories. |
| `semantic_packs.conformance_output_formats` | array | Supported `nose semantic-pack check --format` values. |
| `semantic_packs.inventory` | array | Supported inventory sources. Version 4 reports `compiled-builtin`. |
| `semantic_packs.inventory_output_formats` | array | Supported `nose semantic-pack inventory --format` values. |
| `semantic_packs.adoption_gates` | array | Supported adoption-gate report sources. Version 4 may report `compiled-builtin`; consumers should treat absence as unsupported. |
| `semantic_packs.adoption_gate_output_formats` | array | Supported `nose semantic-pack adoption-gates --format` values. |
| `semantic_packs.compatibility` | array | Supported compatibility report sources. Version 4 may report `policy`; consumers should treat absence as unsupported. |
| `semantic_packs.compatibility_output_formats` | array | Supported `nose semantic-pack compatibility --format` values. |
| `semantic_packs.trust` | array | Supported trust policy labels. |
| `semantic_packs.external_packs_enabled_by_default` | boolean | Always `false`; external packs require explicit CLI/config opt-in. |
| `semantic_packs.external_pack_influence` | string | Current influence of loaded external packs, `metadata-only`. |
| `semantic_packs.external_influence_blockers` | array | Stable blocker labels that currently prevent external rows from influencing analysis. |
| `semantic_packs.external_pack_execution` | string | Current external pack execution support. Version 4 reports `none`; local external packs do not run recognizers, parser/lowering plugins, producer code, sandboxed code, or fixture contents. |
| `il.output_formats` | array | Supported `nose il --format` values. |
| `il.normalized` | boolean | Whether `nose il --normalized` is supported. |
| `il.cfg_norm_toggle` | boolean | Whether `nose il --no-cfg-norm` is supported. |
| `stats.output_formats` | array | Supported stats output formats. |

Known unsupported capabilities or query interfaces should be represented as
`false` when nose has a stable key for them. Unknown keys should be ignored by
consumers. New fields may be added to existing objects without changing
`schema_version`; changing a documented field's type or meaning requires a new
capabilities schema version.

## Query Capability Flags

Version 4 defines these `query.capabilities` keys:

| key | meaning |
|---|---|
| `base_divergence` | `base=<ref>` divergent-edit analysis is supported. |
| `baseline` | `--baseline` and `--write-baseline` are supported. |
| `baseline_changed_detection` | Baseline comparisons can classify changed and resolved families. |
| `baseline_member_digest` | Baselines use accepted member source digests, so reshaped accepted families stay hidden while edited members report as changed. |
| `cache` | `--cache-dir` file analysis caching is supported. |
| `ci_fail_gate` | `--fail-on any|new` gate behavior is supported. |
| `family_drilldown` | Opening a family with `id=<fam>` / `at=FILE:LINE` is supported. |
| `inline_suppression` | Source-level `nose-ignore` markers are supported. |
| `multi_root` | `nose query --root <path>` / `-r <path>` repeatable root analysis is supported. |
| `reinvented_view` | The `reinvented` query view is supported. |
| `semantic_pack_loading` | local semantic-pack v0 manifest files/directories can be loaded for metadata validation. |
| `structured_ignores` | `nose.ignore.json` / `--ignore-file` audited suppressions are supported. |
