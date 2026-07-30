#!/usr/bin/env python3
"""Pinned auxiliary-tool policy, diagnostics, bootstrap, and update checks."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
from pathlib import Path, PurePosixPath
from typing import Any, NoReturn


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "tools" / "auxiliary-tools.json"
SCHEMA = "nose.auxiliary-tools.v1"
SUPPORTED_PLATFORMS = {
    ("darwin", "aarch64"): "darwin-aarch64",
    ("darwin", "arm64"): "darwin-aarch64",
    ("darwin", "x86_64"): "darwin-x86_64",
    ("linux", "aarch64"): "linux-aarch64",
    ("linux", "arm64"): "linux-aarch64",
    ("linux", "x86_64"): "linux-x86_64",
    ("linux", "amd64"): "linux-x86_64",
}
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMAND_TIMEOUT_SECONDS = 5
INSTALL_VERIFY_TIMEOUT_SECONDS = 30


class PolicyError(ValueError):
    """Raised when the checked auxiliary-tool policy is malformed or drifts."""


def fail(message: str, status: int = 2) -> NoReturn:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(status)


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise PolicyError(f"duplicate JSON key: {key!r}")
        result[key] = value
    return result


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_keys,
        )
    except (OSError, json.JSONDecodeError, PolicyError) as error:
        raise PolicyError(f"cannot load {path}: {error}") from error
    if not isinstance(value, dict):
        raise PolicyError(f"{path} root must be an object")
    return value


def require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise PolicyError(f"{context} must be an object")
    return value


def require_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise PolicyError(f"{context} must be a non-empty string")
    return value


def require_string_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise PolicyError(f"{context} must be a non-empty list")
    for index, item in enumerate(value):
        require_string(item, f"{context}[{index}]")
    return value


def require_keys(
    value: dict[str, Any],
    required: set[str],
    allowed: set[str],
    context: str,
) -> None:
    missing = sorted(required - value.keys())
    unknown = sorted(value.keys() - allowed)
    if missing:
        raise PolicyError(f"{context} is missing fields: {', '.join(missing)}")
    if unknown:
        raise PolicyError(f"{context} has unknown fields: {', '.join(unknown)}")


def version_tuple(value: str) -> tuple[int, int, int]:
    if not VERSION_RE.fullmatch(value):
        raise PolicyError(f"version must be X.Y.Z: {value!r}")
    return tuple(int(part) for part in value.split("."))  # type: ignore[return-value]


def validate_manifest(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    require_keys(
        manifest,
        {"schema", "python", "required_commands", "ci_installer", "ci_groups", "tools"},
        {"schema", "python", "required_commands", "ci_installer", "ci_groups", "tools"},
        "manifest",
    )
    if manifest["schema"] != SCHEMA:
        raise PolicyError(f"manifest schema must be {SCHEMA!r}")

    python_policy = require_object(manifest["python"], "python")
    require_keys(
        python_policy,
        {"command", "minimum", "version_args", "version_pattern"},
        {"command", "minimum", "version_args", "version_pattern"},
        "python",
    )
    require_string(python_policy["command"], "python.command")
    version_tuple(require_string(python_policy["minimum"], "python.minimum"))
    require_string_list(python_policy["version_args"], "python.version_args")
    try:
        re.compile(require_string(python_policy["version_pattern"], "python.version_pattern"))
    except re.error as error:
        raise PolicyError(f"python.version_pattern is invalid: {error}") from error

    required_commands = manifest["required_commands"]
    if not isinstance(required_commands, list) or not required_commands:
        raise PolicyError("required_commands must be a non-empty list")
    required_names: set[str] = set()
    for index, item in enumerate(required_commands):
        command = require_object(item, f"required_commands[{index}]")
        require_keys(
            command,
            {"command", "purpose"},
            {"command", "purpose"},
            f"required_commands[{index}]",
        )
        name = require_string(command["command"], f"required_commands[{index}].command")
        require_string(command["purpose"], f"required_commands[{index}].purpose")
        if name in required_names:
            raise PolicyError(f"duplicate required command: {name}")
        required_names.add(name)
    expected_required_commands = {"cargo", "git", "node", "rustup"}
    if required_names != expected_required_commands:
        raise PolicyError(
            "required_commands must cover exactly the external full-gate commands: "
            + ", ".join(sorted(expected_required_commands))
        )

    installer = require_object(manifest["ci_installer"], "ci_installer")
    require_keys(
        installer,
        {"repository", "version", "ref"},
        {"repository", "version", "ref"},
        "ci_installer",
    )
    repository = require_string(installer["repository"], "ci_installer.repository")
    installer_version = require_string(installer["version"], "ci_installer.version")
    version_tuple(installer_version)
    if installer["ref"] != f"v{installer_version}":
        raise PolicyError("ci_installer.ref must be v + ci_installer.version")
    if repository != "taiki-e/install-action":
        raise PolicyError("ci_installer.repository must remain taiki-e/install-action")

    tools = manifest["tools"]
    if not isinstance(tools, list) or not tools:
        raise PolicyError("tools must be a non-empty list")
    tools_by_id: dict[str, dict[str, Any]] = {}
    commands: set[str] = set()
    for index, value in enumerate(tools):
        context = f"tools[{index}]"
        tool = require_object(value, context)
        require_keys(
            tool,
            {
                "id",
                "command",
                "version",
                "policy",
                "version_args",
                "version_pattern",
                "install",
                "update",
            },
            {
                "id",
                "command",
                "version",
                "policy",
                "version_args",
                "version_pattern",
                "install",
                "update",
            },
            context,
        )
        tool_id = require_string(tool["id"], f"{context}.id")
        command = require_string(tool["command"], f"{context}.command")
        version = require_string(tool["version"], f"{context}.version")
        version_tuple(version)
        if tool["policy"] != "exact":
            raise PolicyError(f"{context}.policy must be 'exact'")
        require_string_list(tool["version_args"], f"{context}.version_args")
        try:
            pattern = re.compile(
                require_string(tool["version_pattern"], f"{context}.version_pattern")
            )
        except re.error as error:
            raise PolicyError(f"{context}.version_pattern is invalid: {error}") from error
        if pattern.groups != 1:
            raise PolicyError(f"{context}.version_pattern must have exactly one capture")
        if tool_id in tools_by_id:
            raise PolicyError(f"duplicate tool id: {tool_id}")
        if command in commands:
            raise PolicyError(f"duplicate tool command: {command}")
        tools_by_id[tool_id] = tool
        commands.add(command)

        install = require_object(tool["install"], f"{context}.install")
        kind = install.get("kind")
        if kind == "cargo":
            require_keys(
                install,
                {"kind", "package", "ci_name"},
                {"kind", "package", "ci_name", "rust_component"},
                f"{context}.install",
            )
            require_string(install["package"], f"{context}.install.package")
            require_string(install["ci_name"], f"{context}.install.ci_name")
            if "rust_component" in install:
                require_string(
                    install["rust_component"],
                    f"{context}.install.rust_component",
                )
        elif kind == "release-archive":
            require_keys(
                install,
                {"kind", "archive_binary", "install_name", "assets"},
                {"kind", "archive_binary", "install_name", "assets"},
                f"{context}.install",
            )
            require_string(install["archive_binary"], f"{context}.install.archive_binary")
            if install["install_name"] != command:
                raise PolicyError(f"{context}.install_name must equal command")
            assets = require_object(install["assets"], f"{context}.install.assets")
            if set(assets) != set(SUPPORTED_PLATFORMS.values()):
                raise PolicyError(
                    f"{context}.install.assets must cover exactly "
                    + ", ".join(sorted(set(SUPPORTED_PLATFORMS.values())))
                )
            for platform_id, asset_value in assets.items():
                asset = require_object(asset_value, f"{context}.assets.{platform_id}")
                require_keys(
                    asset,
                    {"url", "sha256"},
                    {"url", "sha256"},
                    f"{context}.assets.{platform_id}",
                )
                url = require_string(asset["url"], f"{context}.assets.{platform_id}.url")
                checksum = require_string(
                    asset["sha256"], f"{context}.assets.{platform_id}.sha256"
                )
                if not url.startswith("https://") or f"/v{version}/" not in url:
                    raise PolicyError(
                        f"{context}.assets.{platform_id}.url must be HTTPS and version-bound"
                    )
                if not SHA256_RE.fullmatch(checksum):
                    raise PolicyError(
                        f"{context}.assets.{platform_id}.sha256 must be lowercase SHA-256"
                    )
        else:
            raise PolicyError(f"{context}.install.kind is unsupported: {kind!r}")

        update = require_object(tool["update"], f"{context}.update")
        update_kind = update.get("kind")
        if update_kind == "crates-io":
            require_keys(
                update,
                {"kind", "crate"},
                {"kind", "crate"},
                f"{context}.update",
            )
            require_string(update["crate"], f"{context}.update.crate")
        elif update_kind == "github-release":
            require_keys(
                update,
                {"kind", "repository"},
                {"kind", "repository"},
                f"{context}.update",
            )
            require_string(update["repository"], f"{context}.update.repository")
        else:
            raise PolicyError(f"{context}.update.kind is unsupported: {update_kind!r}")

    groups = require_object(manifest["ci_groups"], "ci_groups")
    expected_groups = {"coverage", "shell-lint", "supply-chain"}
    if set(groups) != expected_groups:
        raise PolicyError(
            "ci_groups must define exactly: " + ", ".join(sorted(expected_groups))
        )
    grouped: set[str] = set()
    for group, tool_ids_value in groups.items():
        tool_ids = require_string_list(tool_ids_value, f"ci_groups.{group}")
        for tool_id in tool_ids:
            if tool_id not in tools_by_id:
                raise PolicyError(f"ci_groups.{group} names unknown tool {tool_id!r}")
            if tool_id in grouped:
                raise PolicyError(f"CI tool {tool_id!r} appears in multiple groups")
            grouped.add(tool_id)
    if grouped != {"cargo-deny", "cargo-llvm-cov", "cargo-machete", "shellcheck"}:
        raise PolicyError("CI groups must cover the hosted prebuilt tool set exactly")
    return tools_by_id


def load_manifest(path: Path = DEFAULT_MANIFEST) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    manifest = read_json(path)
    return manifest, validate_manifest(manifest)


def parse_only(value: str | None, tools_by_id: dict[str, dict[str, Any]]) -> list[str]:
    if value is None:
        return sorted(tools_by_id)
    selected = [item.strip() for item in value.split(",") if item.strip()]
    if not selected:
        raise PolicyError("--only must name at least one tool")
    duplicates = sorted({item for item in selected if selected.count(item) > 1})
    unknown = sorted(set(selected) - tools_by_id.keys())
    if duplicates:
        raise PolicyError("--only repeats tools: " + ", ".join(duplicates))
    if unknown:
        raise PolicyError("--only names unknown tools: " + ", ".join(unknown))
    return selected


def extract_version(output: str, pattern: str) -> str | None:
    match = re.search(pattern, output)
    return match.group(1) if match else None


def evaluate_version(
    *,
    expected: str,
    pattern: str,
    output: str | None,
    minimum: bool = False,
) -> tuple[str, str | None]:
    if output is None:
        return "missing", None
    observed = extract_version(output, pattern)
    if observed is None:
        return "unrecognized", None
    try:
        observed_tuple = version_tuple(observed)
        expected_tuple = version_tuple(expected)
    except PolicyError:
        return "unrecognized", observed
    if minimum:
        return ("ok" if observed_tuple >= expected_tuple else "too-old"), observed
    return ("ok" if observed_tuple == expected_tuple else "mismatch"), observed


def probe(command: str, args: list[str]) -> tuple[str | None, str | None]:
    resolved = shutil.which(command)
    if resolved is None:
        return None, None
    try:
        completed = subprocess.run(
            [resolved, *args],
            check=False,
            capture_output=True,
            text=True,
            timeout=COMMAND_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return "", str(error)
    output = "\n".join(item for item in (completed.stdout, completed.stderr) if item)
    return output.strip(), None


def action_for(tool_id: str) -> str:
    return f"./scripts/bootstrap-tools.sh --only {tool_id}"


def tool_result(tool: dict[str, Any]) -> dict[str, Any]:
    output, error = probe(tool["command"], tool["version_args"])
    status, observed = evaluate_version(
        expected=tool["version"],
        pattern=tool["version_pattern"],
        output=output,
    )
    return {
        "id": tool["id"],
        "command": tool["command"],
        "status": status if error is None else "probe-error",
        "expected": tool["version"],
        "observed": observed,
        "action": None if status == "ok" and error is None else action_for(tool["id"]),
        "detail": error,
    }


def parse_rust_dev_pin() -> str:
    text = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    match = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if match is None:
        raise PolicyError("rust-toolchain.toml has no channel")
    return match.group(1)


def parse_msrv_pin() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^\s*rust-version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if match is None:
        raise PolicyError("Cargo.toml has no rust-version")
    value = match.group(1)
    if value.count(".") == 1:
        value += ".0"
    version_tuple(value)
    return value


def required_rust_components(manifest: dict[str, Any]) -> list[str]:
    text = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    match = re.search(r"^\s*components\s*=\s*\[([^\]]*)\]", text, re.MULTILINE)
    if match is None:
        raise PolicyError("rust-toolchain.toml has no components list")
    components = re.findall(r'"([^"]+)"', match.group(1))
    components.extend(
        tool["install"]["rust_component"]
        for tool in manifest["tools"]
        if "rust_component" in tool["install"]
    )
    return sorted(set(components))


def toolchain_present(command: str, args: list[str], expected: str) -> tuple[bool, str | None]:
    output, error = probe(command, args)
    if output is None:
        return False, "command missing"
    if error is not None:
        return False, error
    return any(
        line == expected
        or line.startswith(expected + " ")
        or line.startswith(expected + "-")
        for line in output.splitlines()
    ), None


def doctor_results(
    manifest: dict[str, Any],
    tools_by_id: dict[str, dict[str, Any]],
    only: str | None,
) -> list[dict[str, Any]]:
    selected = parse_only(only, tools_by_id)
    results = [tool_result(tools_by_id[tool_id]) for tool_id in selected]
    if only is not None:
        return results

    python_policy = manifest["python"]
    python_output = f"Python {platform.python_version()}"
    status, observed = evaluate_version(
        expected=python_policy["minimum"],
        pattern=python_policy["version_pattern"],
        output=python_output,
        minimum=True,
    )
    results.insert(
        0,
        {
            "id": "python",
            "command": python_policy["command"],
            "status": status,
            "expected": f">={python_policy['minimum']}",
            "observed": observed,
            "action": None if status == "ok" else "./scripts/bootstrap-tools.sh",
            "detail": None,
        },
    )
    for item in manifest["required_commands"]:
        present = shutil.which(item["command"]) is not None
        results.append(
            {
                "id": item["command"],
                "command": item["command"],
                "status": "ok" if present else "missing",
                "expected": "present",
                "observed": "present" if present else None,
                "action": None if present else f"install {item['command']} before bootstrap",
                "detail": item["purpose"],
            }
        )

    dev_pin = parse_rust_dev_pin()
    msrv_pin = parse_msrv_pin()
    for identifier, pin in (("rust-dev-toolchain", dev_pin), ("rust-msrv-toolchain", msrv_pin)):
        present, detail = toolchain_present("rustup", ["toolchain", "list"], pin)
        results.append(
            {
                "id": identifier,
                "command": "rustup",
                "status": "ok" if present else "missing",
                "expected": pin,
                "observed": pin if present else None,
                "action": None if present else "./scripts/bootstrap-tools.sh --with-toolchains",
                "detail": detail,
            }
        )

    component_output, component_error = probe(
        "rustup",
        ["component", "list", "--toolchain", dev_pin, "--installed"],
    )
    component_lines = [] if component_output is None else component_output.splitlines()
    for component in required_rust_components(manifest):
        observed_component = component.removesuffix("-preview")
        present = any(
            line == observed_component
            or line.startswith(observed_component + " ")
            or line.startswith(observed_component + "-")
            for line in component_lines
        )
        results.append(
            {
                "id": f"rust-component-{component}",
                "command": "rustup",
                "status": "ok" if present and component_error is None else "missing",
                "expected": f"{component}@{dev_pin}",
                "observed": observed_component if present else None,
                "action": (
                    None
                    if present and component_error is None
                    else "./scripts/bootstrap-tools.sh --with-toolchains"
                ),
                "detail": component_error,
            }
        )

    lean_pin = (ROOT / "lean-toolchain").read_text(encoding="utf-8").strip()
    present, detail = toolchain_present("elan", ["toolchain", "list"], lean_pin)
    results.append(
        {
            "id": "lean-toolchain",
            "command": "elan",
            "status": "ok" if present else "missing",
            "expected": lean_pin,
            "observed": lean_pin if present else None,
            "action": None if present else "./scripts/bootstrap-tools.sh --with-toolchains",
            "detail": detail,
        }
    )
    return results


def print_doctor(results: list[dict[str, Any]], as_json: bool) -> int:
    if as_json:
        print(json.dumps({"results": results}, indent=2, sort_keys=True))
    else:
        for result in results:
            observed = result["observed"] or "-"
            print(
                f"{result['status']:>11}  {result['id']:<22} "
                f"expected={result['expected']} observed={observed}"
            )
            if result["detail"] and result["status"] != "ok":
                print(f"             detail: {result['detail']}")
            if result["action"]:
                print(f"             fix: {result['action']}")
    return 0 if all(result["status"] == "ok" for result in results) else 1


def ci_tool_spec(tool: dict[str, Any]) -> str:
    install = tool["install"]
    name = install["ci_name"] if install["kind"] == "cargo" else tool["id"]
    return f"{name}@{tool['version']}"


def append_github_output(path: Path, key: str, value: str) -> None:
    with path.open("a", encoding="utf-8") as handle:
        handle.write(f"{key}={value}\n")


def platform_id(override: str | None) -> str:
    if override is not None:
        if override not in set(SUPPORTED_PLATFORMS.values()):
            raise PolicyError(f"unsupported platform: {override}")
        return override
    key = (platform.system().lower(), platform.machine().lower())
    try:
        return SUPPORTED_PLATFORMS[key]
    except KeyError as error:
        raise PolicyError(
            f"unsupported host platform: {key[0]}-{key[1]}"
        ) from error


def verify_download(path: Path, expected: str) -> None:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    observed = digest.hexdigest()
    if observed != expected:
        raise PolicyError(
            f"download checksum mismatch: expected {expected}, observed {observed}"
        )


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "nose-auxiliary-tool-bootstrap/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            with destination.open("wb") as output:
                shutil.copyfileobj(response, output)
    except (OSError, urllib.error.URLError) as error:
        raise PolicyError(f"cannot download {url}: {error}") from error


def extract_named_binary(archive: Path, archive_binary: str, destination: Path) -> None:
    try:
        with tarfile.open(archive, mode="r:*") as bundle:
            matches = []
            for member in bundle.getmembers():
                member_path = PurePosixPath(member.name)
                if member_path.is_absolute() or ".." in member_path.parts:
                    raise PolicyError(f"unsafe archive path: {member.name}")
                if member.islnk() or member.issym() or member.isdev():
                    raise PolicyError(f"unsafe archive member: {member.name}")
                if member.isfile() and member_path.name == archive_binary:
                    matches.append(member)
            if len(matches) != 1:
                raise PolicyError(
                    f"archive must contain exactly one {archive_binary!r}; "
                    f"found {len(matches)}"
                )
            source = bundle.extractfile(matches[0])
            if source is None:
                raise PolicyError(f"cannot read {matches[0].name} from archive")
            with destination.open("wb") as output:
                shutil.copyfileobj(source, output)
    except (OSError, tarfile.TarError) as error:
        raise PolicyError(f"cannot extract {archive}: {error}") from error


def atomic_install(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        dir=destination.parent,
        prefix=f".{destination.name}.",
        delete=False,
    ) as output:
        temporary = Path(output.name)
        with source.open("rb") as input_handle:
            shutil.copyfileobj(input_handle, output)
    try:
        temporary.chmod(
            stat.S_IRUSR
            | stat.S_IWUSR
            | stat.S_IXUSR
            | stat.S_IRGRP
            | stat.S_IXGRP
            | stat.S_IROTH
            | stat.S_IXOTH
        )
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def install_release_tool(tool: dict[str, Any], selected_platform: str, bin_dir: Path) -> None:
    install = tool["install"]
    asset = install["assets"][selected_platform]
    suffix = ".tar.xz" if asset["url"].endswith(".tar.xz") else ".tar.gz"
    with tempfile.TemporaryDirectory(prefix=f"nose-{tool['id']}-") as temp_name:
        temp = Path(temp_name)
        archive = temp / f"download{suffix}"
        binary = temp / install["install_name"]
        print(f"download  {tool['id']} {tool['version']} ({selected_platform})")
        download(asset["url"], archive)
        verify_download(archive, asset["sha256"])
        extract_named_binary(archive, install["archive_binary"], binary)
        atomic_install(binary, bin_dir / install["install_name"])


def install_cargo_tool(tool: dict[str, Any], bin_dir: Path, dev_pin: str) -> None:
    install = tool["install"]
    if shutil.which("cargo") is None:
        raise PolicyError("cargo is required to install cargo-based auxiliary tools")
    if shutil.which("rustup") is None:
        raise PolicyError("rustup is required to use the pinned development toolchain")
    with tempfile.TemporaryDirectory(prefix=f"nose-{tool['id']}-") as temp_name:
        install_root = Path(temp_name) / "cargo-root"
        command = [
            "cargo",
            f"+{dev_pin}",
            "install",
            "--locked",
            "--version",
            tool["version"],
            "--root",
            str(install_root),
            install["package"],
        ]
        print("run       " + " ".join(command))
        subprocess.run(command, cwd=ROOT, check=True)
        source = install_root / "bin" / tool["command"]
        if not source.is_file():
            raise PolicyError(f"cargo install did not produce {source}")
        atomic_install(source, bin_dir / tool["command"])


def bootstrap_toolchains(manifest: dict[str, Any], elan_path: Path) -> None:
    if shutil.which("rustup") is None:
        raise PolicyError("rustup is required before --with-toolchains")
    dev_pin = parse_rust_dev_pin()
    msrv_pin = parse_msrv_pin()
    for pin in (dev_pin, msrv_pin):
        print(f"toolchain rust {pin}")
        subprocess.run(["rustup", "toolchain", "install", pin], check=True)
    subprocess.run(
        [
            "rustup",
            "component",
            "add",
            *required_rust_components(manifest),
            "--toolchain",
            dev_pin,
        ],
        check=True,
    )
    elan_tool = next(tool for tool in manifest["tools"] if tool["id"] == "elan")
    elan_result = tool_result_with_path(elan_tool, elan_path)
    if elan_result["status"] != "ok":
        raise PolicyError(
            "the exact pinned elan must be bootstrapped before Lean toolchain setup"
        )
    lean_pin = (ROOT / "lean-toolchain").read_text(encoding="utf-8").strip()
    print(f"toolchain Lean {lean_pin}")
    subprocess.run([str(elan_path), "toolchain", "install", lean_pin], check=True)


def bootstrap(
    manifest: dict[str, Any],
    tools_by_id: dict[str, dict[str, Any]],
    args: argparse.Namespace,
) -> int:
    selected = parse_only(args.only, tools_by_id)
    selected_platform = platform_id(args.platform)
    bin_dir = args.bin_dir.expanduser().resolve()
    dev_pin = parse_rust_dev_pin()
    for tool_id in selected:
        tool = tools_by_id[tool_id]
        destination = bin_dir / tool["command"]
        result = (
            tool_result_with_path(tool, destination)
            if destination.is_file()
            else {"status": "missing", "observed": None}
        )
        if result["status"] == "ok":
            print(f"skip      {tool_id} {tool['version']} (already at {destination})")
            continue
        install = tool["install"]
        if args.dry_run:
            if install["kind"] == "cargo":
                print(
                    f"would run cargo +{dev_pin} install --locked --version "
                    f"{tool['version']} {install['package']}"
                )
            else:
                asset = install["assets"][selected_platform]
                print(
                    f"would install {tool_id} {tool['version']} from "
                    f"{asset['url']} (sha256 {asset['sha256']})"
                )
            continue
        if install["kind"] == "cargo":
            install_cargo_tool(tool, bin_dir, dev_pin)
        else:
            install_release_tool(tool, selected_platform, bin_dir)
        installed = tool_result_with_path(tool, bin_dir / tool["command"])
        if installed["status"] != "ok":
            raise PolicyError(
                f"installed {tool_id} failed version verification: "
                f"{installed['status']} ({installed['observed']})"
            )
        print(f"installed {tool_id} {tool['version']} -> {bin_dir / tool['command']}")
    if args.with_toolchains:
        if args.dry_run:
            print(
                f"would install Rust {dev_pin}, Rust {parse_msrv_pin()}, "
                f"Rust components {','.join(required_rust_components(manifest))}, "
                f"and Lean {(ROOT / 'lean-toolchain').read_text().strip()}"
            )
        else:
            elan_path = bin_dir / tools_by_id["elan"]["command"]
            if not elan_path.is_file():
                resolved_elan = shutil.which(tools_by_id["elan"]["command"])
                if resolved_elan is None:
                    raise PolicyError(
                        "--with-toolchains needs pinned elan; include elan in --only"
                    )
                elan_path = Path(resolved_elan)
            bootstrap_toolchains(manifest, elan_path)
    if not args.dry_run and str(bin_dir) not in os.environ.get("PATH", "").split(os.pathsep):
        print(f"note: add {bin_dir} to PATH; bootstrap does not edit shell configuration")
    return 0


def tool_result_with_path(tool: dict[str, Any], command_path: Path) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            [str(command_path), *tool["version_args"]],
            check=False,
            capture_output=True,
            text=True,
            timeout=INSTALL_VERIFY_TIMEOUT_SECONDS,
        )
        output = "\n".join(
            item for item in (completed.stdout, completed.stderr) if item
        ).strip()
        error = None
    except (OSError, subprocess.TimeoutExpired) as caught:
        output = ""
        error = str(caught)
    status, observed = evaluate_version(
        expected=tool["version"],
        pattern=tool["version_pattern"],
        output=output,
    )
    return {
        "status": status if error is None else "probe-error",
        "observed": observed,
    }


def policy_files() -> list[Path]:
    return [
        ROOT / ".github" / "workflows" / "ci.yml",
        ROOT / ".github" / "workflows" / "corpus-verify.yml",
        ROOT / "docs" / "contributing.md",
        ROOT / "docs" / "tooling.md",
        ROOT / "scripts" / "check-ci-local.sh",
        ROOT / "scripts" / "check-docs.sh",
        ROOT / "scripts" / "bootstrap-tools.sh",
        ROOT / ".github" / "workflows" / "tool-updates.yml",
    ]


def check_repository_policy(manifest: dict[str, Any]) -> None:
    installer = manifest["ci_installer"]
    python_minimum = manifest["python"]["minimum"]
    python_tuple = ", ".join(str(part) for part in version_tuple(python_minimum))
    expected_action = f"{installer['repository']}@{installer['ref']}"
    workflow_paths = sorted((ROOT / ".github" / "workflows").glob("*.yml"))
    workflow_text = "\n".join(path.read_text(encoding="utf-8") for path in workflow_paths)
    refs = set(re.findall(r"taiki-e/install-action@([^\s]+)", workflow_text))
    if refs != {installer["ref"]}:
        raise PolicyError(
            f"install-action refs must be exactly {installer['ref']!r}; found {sorted(refs)}"
        )
    ci_text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    if ci_text.count(f"uses: {expected_action}") != 3:
        raise PolicyError("ci.yml must use the pinned installer exactly three times")

    required_tokens = {
        ROOT / ".github" / "workflows" / "ci.yml": [
            "ci-spec shell-lint",
            "ci-spec coverage",
            "ci-spec supply-chain",
            "bootstrap-tools.sh --only awiki",
            "bootstrap-tools.sh --only elan",
            "doctor --only awiki",
            "doctor --only elan",
            "gate · aux-tool-policy",
        ],
        ROOT / ".github" / "workflows" / "corpus-verify.yml": [
            "bootstrap-tools.sh --only elan",
            "doctor --only elan",
        ],
        ROOT / ".github" / "workflows" / "tool-updates.yml": [
            "matrix:",
            "ubuntu-latest",
            "macos-latest",
            "bootstrap-tools.sh --only awiki,elan,shellcheck",
            "doctor --only awiki,elan,shellcheck",
            "check-updates",
            "darwin-aarch64",
            "darwin-x86_64",
            "linux-aarch64",
            "linux-x86_64",
        ],
        ROOT / "docs" / "contributing.md": [
            "./scripts/aux_tools.py doctor",
            "./scripts/bootstrap-tools.sh",
            "Node.js",
            "tooling.md",
        ],
        ROOT / "docs" / "tooling.md": [
            "tools/auxiliary-tools.json",
            "./scripts/aux_tools.py check-updates",
            "Node.js",
        ],
        ROOT / "scripts" / "check-ci-local.sh": [
            "audit_aux_tools awiki",
            "audit_aux_tools elan",
            "audit_aux_tools shellcheck",
            "audit_aux_tools cargo-llvm-cov",
            "audit_aux_tools cargo-machete,cargo-deny",
        ],
    }
    for path, tokens in required_tokens.items():
        if not path.is_file():
            raise PolicyError(f"required policy consumer is missing: {path.relative_to(ROOT)}")
        text = path.read_text(encoding="utf-8")
        missing = [token for token in tokens if token not in text]
        if missing:
            raise PolicyError(
                f"{path.relative_to(ROOT)} does not consume policy tokens: {missing}"
            )

    for path in (
        ROOT / "docs" / "contributing.md",
        ROOT / "docs" / "tooling.md",
    ):
        if f"Python {python_minimum} or newer" not in path.read_text(encoding="utf-8"):
            raise PolicyError(
                f"{path.relative_to(ROOT)} does not name Python floor {python_minimum}"
            )
    for path in (
        ROOT / "scripts" / "check-ci-local.sh",
        ROOT / "scripts" / "bootstrap-tools.sh",
    ):
        text = path.read_text(encoding="utf-8")
        if f"sys.version_info < ({python_tuple})" not in text:
            raise PolicyError(
                f"{path.relative_to(ROOT)} does not enforce Python floor {python_minimum}"
            )
    bootstrap_text = (ROOT / "scripts" / "bootstrap-tools.sh").read_text(
        encoding="utf-8"
    )
    if 'if [[ "$dry_run" -eq 1 ]]' not in bootstrap_text:
        raise PolicyError("bootstrap dry-run must not install a missing Python")

    forbidden_patterns = {
        r"leanprover/elan/master/elan-init\.sh": "moving elan installer",
        r"awiki(?:/cmd/awiki)?@latest": "moving awiki version",
        r"go install github\.com/corca-ai/awiki": "out-of-band awiki install",
        r"brew install (?:shellcheck|corca-ai/tap/awiki)": "out-of-band tool install",
        r"cargo install (?:cargo-deny|cargo-machete|cargo-llvm-cov)": (
            "out-of-band cargo tool install"
        ),
        r"actions/setup-go@": "obsolete Go setup used only for awiki",
        r"taiki-e/install-action@v2(?:\s|$)": "major-only installer ref",
    }
    for path in policy_files():
        if not path.is_file():
            raise PolicyError(f"policy file is missing: {path.relative_to(ROOT)}")
        text = path.read_text(encoding="utf-8")
        for pattern, reason in forbidden_patterns.items():
            if re.search(pattern, text):
                raise PolicyError(f"{path.relative_to(ROOT)} contains {reason}")


def latest_version(tool: dict[str, Any]) -> str:
    update = tool["update"]
    headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": "nose-auxiliary-tool-update-check/1",
    }
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if update["kind"] == "github-release":
        url = f"https://api.github.com/repos/{update['repository']}/releases/latest"
        field = "tag_name"
    else:
        url = f"https://crates.io/api/v1/crates/{update['crate']}"
        field = "crate.max_stable_version"
        headers["User-Agent"] = "nose (https://github.com/corca-ai/nose)"
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.load(response)
    except (OSError, urllib.error.URLError, json.JSONDecodeError) as error:
        raise PolicyError(f"cannot check updates for {tool['id']}: {error}") from error
    if field == "tag_name":
        value = payload.get("tag_name")
    else:
        value = payload.get("crate", {}).get("max_stable_version")
    if not isinstance(value, str):
        raise PolicyError(f"update response for {tool['id']} has no version")
    value = value.removeprefix("v")
    version_tuple(value)
    return value


def command_list(manifest: dict[str, Any], tools_by_id: dict[str, dict[str, Any]]) -> int:
    print(
        f"{'tool':<18} {'policy':<12} {'version':<10} {'installer':<16}"
    )
    python_policy = manifest["python"]
    print(
        f"{'python':<18} {'minimum':<12} {python_policy['minimum']:<10} {'platform':<16}"
    )
    for tool_id in sorted(tools_by_id):
        tool = tools_by_id[tool_id]
        print(
            f"{tool_id:<18} {tool['policy']:<12} {tool['version']:<10} "
            f"{tool['install']['kind']:<16}"
        )
    return 0


def selftest(manifest: dict[str, Any], tools_by_id: dict[str, dict[str, Any]]) -> int:
    cases = [
        (
            "missing",
            evaluate_version(
                expected="1.2.3", pattern=r"tool (\d+\.\d+\.\d+)", output=None
            )[0],
            "missing",
        ),
        (
            "exact mismatch",
            evaluate_version(
                expected="1.2.3",
                pattern=r"tool (\d+\.\d+\.\d+)",
                output="tool 1.2.2",
            )[0],
            "mismatch",
        ),
        (
            "exact acceptable",
            evaluate_version(
                expected="1.2.3",
                pattern=r"tool (\d+\.\d+\.\d+)",
                output="tool 1.2.3",
            )[0],
            "ok",
        ),
        (
            "minimum old",
            evaluate_version(
                expected="3.10.0",
                pattern=r"Python (\d+\.\d+\.\d+)",
                output="Python 3.9.18",
                minimum=True,
            )[0],
            "too-old",
        ),
        (
            "minimum acceptable",
            evaluate_version(
                expected="3.10.0",
                pattern=r"Python (\d+\.\d+\.\d+)",
                output="Python 3.14.0",
                minimum=True,
            )[0],
            "ok",
        ),
        (
            "unrecognized",
            evaluate_version(
                expected="1.2.3", pattern=r"tool (\d+\.\d+\.\d+)", output="unknown"
            )[0],
            "unrecognized",
        ),
    ]
    for name, observed, expected in cases:
        if observed != expected:
            raise PolicyError(
                f"selftest {name!r}: expected {expected!r}, observed {observed!r}"
            )
    for target in sorted(set(SUPPORTED_PLATFORMS.values())):
        if platform_id(target) != target:
            raise PolicyError(f"selftest platform resolution failed for {target}")
        for tool in tools_by_id.values():
            if tool["install"]["kind"] == "release-archive":
                _ = tool["install"]["assets"][target]

    mutated = copy.deepcopy(manifest)
    mutated["tools"][0]["version"] = "latest"
    try:
        validate_manifest(mutated)
    except PolicyError:
        pass
    else:
        raise PolicyError("selftest accepted a moving tool version")

    with tempfile.TemporaryDirectory(prefix="nose-aux-policy-") as temp_name:
        duplicate_path = Path(temp_name) / "duplicate.json"
        duplicate_path.write_text('{"schema": 1, "schema": 2}', encoding="utf-8")
        try:
            read_json(duplicate_path)
        except PolicyError:
            pass
        else:
            raise PolicyError("selftest accepted a duplicate JSON key")
    print("auxiliary-tool self-tests passed")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help=argparse.SUPPRESS,
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("selftest", help="run isolated policy and probe self-tests")
    subparsers.add_parser(
        "check-policy",
        help="validate the manifest and repository consumers without network access",
    )
    subparsers.add_parser("list", help="render the checked versions")

    doctor_parser = subparsers.add_parser(
        "doctor",
        help="read-only diagnosis of local auxiliary tools and toolchains",
    )
    doctor_parser.add_argument("--only", help="comma-separated tool ids")
    doctor_parser.add_argument("--json", action="store_true", help="emit JSON")

    spec_parser = subparsers.add_parser(
        "ci-spec",
        help="render exact tool@version specs for one hosted CI group",
    )
    spec_parser.add_argument("group")
    spec_parser.add_argument("--github-output", type=Path)

    bootstrap_parser = subparsers.add_parser(
        "bootstrap",
        help="explicitly install pinned tools without editing user configuration",
    )
    bootstrap_parser.add_argument("--only", help="comma-separated tool ids")
    bootstrap_parser.add_argument(
        "--bin-dir",
        type=Path,
        default=Path.home() / ".local" / "bin",
    )
    bootstrap_parser.add_argument("--dry-run", action="store_true")
    bootstrap_parser.add_argument(
        "--platform",
        choices=sorted(set(SUPPORTED_PLATFORMS.values())),
        help="override the host only for reproducible dry-run testing",
    )
    bootstrap_parser.add_argument("--with-toolchains", action="store_true")

    update_parser = subparsers.add_parser(
        "check-updates",
        help="read release APIs and report newer compatible pins",
    )
    update_parser.add_argument("--fail-on-update", action="store_true")
    update_parser.add_argument("--markdown", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        manifest, tools_by_id = load_manifest(args.manifest)
        if args.command == "selftest":
            return selftest(manifest, tools_by_id)
        if args.command == "check-policy":
            check_repository_policy(manifest)
            print("auxiliary-tool policy is synchronized")
            return 0
        if args.command == "list":
            return command_list(manifest, tools_by_id)
        if args.command == "doctor":
            return print_doctor(
                doctor_results(manifest, tools_by_id, args.only),
                args.json,
            )
        if args.command == "ci-spec":
            groups = manifest["ci_groups"]
            if args.group not in groups:
                raise PolicyError(
                    f"unknown CI group {args.group!r}; choose from "
                    + ", ".join(sorted(groups))
                )
            value = ",".join(
                ci_tool_spec(tools_by_id[tool_id]) for tool_id in groups[args.group]
            )
            print(value)
            if args.github_output:
                append_github_output(args.github_output, "tools", value)
            return 0
        if args.command == "bootstrap":
            return bootstrap(manifest, tools_by_id, args)
        if args.command == "check-updates":
            rows = []
            updates = 0
            for tool_id in sorted(tools_by_id):
                current = tools_by_id[tool_id]["version"]
                latest = latest_version(tools_by_id[tool_id])
                status = "update" if version_tuple(latest) > version_tuple(current) else "current"
                updates += status == "update"
                rows.append((tool_id, current, latest, status))
            if args.markdown:
                print("| Tool | Pinned | Latest | Status |")
                print("|---|---:|---:|---|")
                for row in rows:
                    print(f"| {row[0]} | {row[1]} | {row[2]} | {row[3]} |")
            else:
                for row in rows:
                    print(
                        f"{row[3]:>7}  {row[0]:<18} pinned={row[1]} latest={row[2]}"
                    )
            if updates and args.fail_on_update:
                return 3
            return 0
        raise AssertionError(f"unhandled command: {args.command}")
    except (PolicyError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
