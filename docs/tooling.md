# Auxiliary development tools

The checked [`tools/auxiliary-tools.json`](../tools/auxiliary-tools.json)
manifest is the single policy authority for development tools that are not
ordinary Cargo workspace dependencies. It owns the Python floor, exact `awiki`,
ShellCheck, `cargo-deny`, `cargo-machete`, `cargo-llvm-cov`, and `elan` pins,
the hosted prebuilt-binary installer ref, and checksummed macOS/Linux release
assets. It also inventories unpinned host prerequisites such as Node.js; those
must be present but are not installed or version-pinned by this repository.

Rust development, MSRV, and Lean toolchain versions remain in their native
owners: [`rust-toolchain.toml`](../rust-toolchain.toml),
[`Cargo.toml`](../Cargo.toml), and [`lean-toolchain`](../lean-toolchain).
The tooling commands read those files directly rather than copying their
versions into another policy file.

## Diagnose without changing the machine

Run the fast, read-only doctor before a full local gate:

```sh
./scripts/aux_tools.py doctor
./scripts/aux_tools.py doctor --json
./scripts/aux_tools.py doctor --only awiki,shellcheck
```

The complete doctor checks command availability, exact auxiliary versions,
Python's minimum version, and the presence of the checked Rust development,
Rust MSRV, and Lean toolchains. A focused `--only` check probes just the named
auxiliary binaries. It executes version/list queries with short timeouts; it
does not access the network, install a package, edit a configuration file, or
download a toolchain.

To inspect the current pins without maintaining a second table:

```sh
./scripts/aux_tools.py list
```

`missing`, `too-old`, `mismatch`, `unrecognized`, and `probe-error` are
diagnostic failures. Each failed auxiliary row points to the focused bootstrap
command that repairs it.

## Install only by explicit request

The separate bootstrap command is the only repository-owned installation
surface:

```sh
./scripts/bootstrap-tools.sh
./scripts/bootstrap-tools.sh --only cargo-deny,awiki
./scripts/bootstrap-tools.sh --with-toolchains
./scripts/bootstrap-tools.sh --dry-run --platform linux-x86_64
```

It is idempotent: a binary at the checked version is skipped. Release binaries
are selected by OS and architecture, downloaded from a version-bound HTTPS
asset, verified against the checked SHA-256 digest, safely extracted, and
atomically written. Cargo tools are built with the checked development Rust
toolchain, `--locked`, and an exact package version. The default destination is
`~/.local/bin`; use `--bin-dir` to choose another directory.

Bootstrap never edits shell startup files, Cargo configuration, Git
configuration, or unrelated files in the destination. If the destination is
not already on `PATH`, it prints the required follow-up instead. On macOS or
apt-based Linux, the shell wrapper can first obtain a supported Python when
Python 3.10.0 or newer is absent; other hosts receive a prerequisite diagnostic.
Node.js, `cargo`, `rustup`, and the host package manager remain explicit
prerequisites when their corresponding path is needed.

`--with-toolchains` additionally installs the versions named by the three
native toolchain owners. Without that option, bootstrap changes auxiliary
binaries only. The ordinary `--fast`, `--full`, and named gate commands never
invoke bootstrap.

## Hosted CI contract

Hosted coverage, shell-lint, and supply-chain jobs ask
`scripts/aux_tools.py ci-spec <group>` for their exact `tool@version` inputs.
The action ref itself is exact and checked against the same manifest. Docs and
formal jobs call the explicit bootstrap for their one release-archive tool,
then run a focused doctor before the ordinary named gate. The Soundness Lab
uses the same pinned `elan` path.

`./scripts/check-ci-local.sh --gate aux-tool-policy` runs isolated
missing/mismatch/acceptable-version tests, validates every supported
macOS/Linux asset mapping, and rejects drift or moving setup references across
the manifest, workflows, bootstrap, and contributor docs. It is a read-only
fast/full/hosted gate.

## Updating a pin

The `Auxiliary tool updates` workflow runs Linux/macOS release-archive bootstrap
smokes on relevant pull requests and every week. It also runs the
network-read-only command:

```sh
./scripts/aux_tools.py check-updates
```

It reports upstream releases and fails visibly when a newer version is
available; it does not modify the repository. Dependabot separately proposes
GitHub Action ref updates. A maintainer reviews compatibility and makes the
coherent policy change:

1. Update the version once in `tools/auxiliary-tools.json`.
2. For a release archive, replace every platform URL and SHA-256 digest with
   the new official assets. For the hosted installer, update its exact
   manifest ref and all three exact workflow refs together.
3. Run the self-test, policy check, and all platform dry runs:

   ```sh
   ./scripts/aux_tools.py selftest
   ./scripts/aux_tools.py check-policy
   ./scripts/aux_tools.py bootstrap --dry-run --platform darwin-aarch64
   ./scripts/aux_tools.py bootstrap --dry-run --platform darwin-x86_64
   ./scripts/aux_tools.py bootstrap --dry-run --platform linux-aarch64
   ./scripts/aux_tools.py bootstrap --dry-run --platform linux-x86_64
   ```

4. Bootstrap the changed tool on a supported host, run its focused doctor, and
   run the gates that consume it before the full repository gate.

Setup paths must not substitute a moving tag or an unversioned direct install.
If an upstream no longer publishes one supported asset, keep the old checked
pin until the platform policy is deliberately changed and reviewed.

The 0.21 preparation refreshes the coverage reporter to 0.9.1 and the Lean
installer to 4.2.4 after the update workflow reported newer releases. The Lean
compiler remains pinned by `lean-toolchain`. Archive digests come from
the official release assets; bootstrap verifies downloaded bytes. Coverage and
formal proof gates must pass with these tool versions before qualification.

Return to the [contributor workflow](contributing.md) for the normal local
sequence. The [repository gate inventory](repository-gates.md) explains how the
policy check participates in every lane.
