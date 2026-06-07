# Capabilities contract

`nose capabilities` emits the stable machine-readable contract for the installed
binary. Use it from installers, editor integrations, CI wrappers, and doctor
commands before invoking `nose scan`. For the human command guide see
[usage](usage.md); for scan result JSON see [scan-json](scan-json.md).

## Why this is not help text

`nose --help` is a human interface. It can change wording, examples, wrapping,
and ordering to improve readability. Tools should not scrape it.

`nose capabilities` is an integration interface. It is JSON-only, has its own
`schema_version`, and reports what the binary supports as data: stable commands,
scan modes, output formats, schema versions, config keys, and capability flags.

Integration rule: branch on `schema_version`, ignore unknown fields, and test capability
flags before passing optional scan arguments. A wrapper that does this can run against older
and newer nose binaries without scraping help text or guessing from the package version.

## Example

```sh
nose capabilities
```

```json
{
  "schema_version": 1,
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
    "stable": ["capabilities", "il", "review", "scan", "stats"]
  },
  "schemas": {
    "capabilities": [1],
    "scan_json": [1]
  },
  "scan": {
    "modes": ["syntax", "semantic", "near"],
    "default_modes": ["syntax", "semantic"],
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
      "sort",
      "top"
    ],
    "capabilities": {
      "baseline": true,
      "baseline_changed_detection": true,
      "cache": true,
      "ci_fail_gate": true,
      "diff": true,
      "hotspots": true,
      "inline_suppression": true,
      "proposal": true,
      "structured_ignores": true
    }
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

`scan.modes` lists stable scan modes only. Hidden experimental modes such as
`abstraction` may be accepted by a development binary without appearing here; wrappers
should treat absence from `scan.modes` as "not stable for automation."

`tool.version` is shown as the `<version>` placeholder because the field always reports the
installed binary's own version (`nose --version`); the example deliberately does not pin a
release so it can't drift.

## Version 1 fields

| field | type | meaning |
|---|---|---|
| `schema_version` | integer | Capabilities contract version. Version 1 is documented here. |
| `tool.name` | string | Always `nose`. |
| `tool.version` | string | Package version of the installed binary. |
| `platform.os` | string | Rust target OS name, such as `linux`, `macos`, or `windows`. |
| `platform.arch` | string | Rust target architecture, such as `x86_64` or `aarch64`. |
| `platform.family` | string | Rust target family, such as `unix` or `windows`. |
| `interfaces.capabilities_json` | boolean | Whether `nose capabilities` is the supported capability query interface. |
| `interfaces.version_json` | boolean | Whether `nose --version --json` is supported. Version 1 reports `false`. |
| `interfaces.doctor_json` | boolean | Whether `nose doctor --json` is supported. Version 1 reports `false`. |
| `commands.stable` | array | Stable user-facing commands that integrations may invoke. Hidden research commands are intentionally omitted. |
| `schemas.capabilities` | array | Supported capabilities schema versions. |
| `schemas.scan_json` | array | Supported `nose scan --format json` schema versions. |
| `scan.modes` | array | Supported `--mode` values. |
| `scan.default_modes` | array | Modes used by `nose scan` when `--mode` is omitted. |
| `scan.output_formats` | array | Supported `nose scan --format` values. |
| `scan.sort_keys` | array | Supported `nose scan --sort` values. |
| `scan.config_keys` | array | Supported `[scan]` keys in `nose.toml` / `.nose.toml`. |
| `scan.capabilities` | object | Stable boolean capability flags for scan workflows. |
| `il.output_formats` | array | Supported `nose il --format` values. |
| `il.normalized` | boolean | Whether `nose il --normalized` is supported. |
| `il.cfg_norm_toggle` | boolean | Whether `nose il --no-cfg-norm` is supported. |
| `stats.output_formats` | array | Supported stats output formats. |

Known unsupported capabilities or query interfaces should be represented as
`false` when nose has a stable key for them. Unknown keys should be ignored by
consumers. New fields may be added to existing objects without changing
`schema_version`; changing a documented field's type or meaning requires a new
capabilities schema version.

## Scan capability flags

Version 1 defines these `scan.capabilities` keys:

| key | meaning |
|---|---|
| `baseline` | `--baseline` and `--write-baseline` are supported. |
| `baseline_changed_detection` | Baseline comparisons can classify changed and resolved families. |
| `cache` | `--cache-dir` file analysis caching is supported. |
| `ci_fail_gate` | `--fail-on any|new` gate behavior is supported. |
| `diff` | the `--show diff` review view is supported. |
| `hotspots` | the `--show hotspots` directory duplicated-line summary is supported. |
| `inline_suppression` | Source-level `nose-ignore` markers are supported. |
| `proposal` | the `--show proposal` extraction-skeleton view is supported. |
| `structured_ignores` | `nose.ignore.json` / `--ignore-file` audited suppressions are supported. |
