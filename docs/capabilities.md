# Capabilities contract

`nose capabilities` emits the stable machine-readable contract for the installed
binary. Use it from installers, editor integrations, CI wrappers, and doctor
commands before invoking `nose scan`. For the human command guide see
[usage](usage.md); for scan result JSON see [scan-json](scan-json.md). Back to
[home](home.md).

## Why this is not help text

`nose --help` is a human interface. It can change wording, examples, wrapping,
and ordering to improve readability. Tools should not scrape it.

`nose capabilities` is an integration interface. It is JSON-only, has its own
`schema_version`, and reports what the binary supports as data: stable commands,
scan modes, output formats, schema versions, config keys, and capability flags.

## Example

```sh
nose capabilities
```

```json
{
  "schema_version": 1,
  "tool": {
    "name": "nose",
    "version": "0.4.0"
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
    "stable": ["capabilities", "il", "scan", "stats"]
  },
  "schemas": {
    "capabilities": [1],
    "scan_json": [1]
  },
  "scan": {
    "modes": ["syntax", "semantic", "near"],
    "default_modes": ["syntax", "semantic"],
    "output_formats": ["human", "json", "markdown", "sarif"],
    "sort_keys": ["extractability", "value", "sites"],
    "config_keys": ["exclude", "ignore-file", "min-lines"],
    "capabilities": {
      "baseline": true,
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

The real `config_keys` and `scan.capabilities` objects may contain more entries
than this shortened example.

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
| `ci_fail_gate` | `--fail` and `--fail-on-new` gate behavior is supported. |
| `diff` | Human `--diff` review output is supported. |
| `hotspots` | `--hotspots` module-level duplicate-line summary is supported. |
| `inline_suppression` | Source-level `nose-ignore` markers are supported. |
| `proposal` | Human `--proposal` extraction skeletons are supported. |
| `structured_ignores` | `nose.ignore.json` / `--ignore-file` audited suppressions are supported. |
