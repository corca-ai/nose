#!/usr/bin/env bash
# Explicit, idempotent installer for the checked auxiliary-tool policy.
# This command installs binaries only; it never edits shell or repository config.
set -euo pipefail
cd "$(dirname "$0")/.."

dry_run=0
for argument in "$@"; do
    if [[ "$argument" == "--dry-run" ]]; then
        dry_run=1
    fi
done

python_command=""
if command -v python3 >/dev/null 2>&1 &&
   python3 -c 'import sys; raise SystemExit(sys.version_info < (3, 10, 0))'; then
    python_command="$(command -v python3)"
fi

if [[ -z "$python_command" ]]; then
    if [[ "$dry_run" -eq 1 ]]; then
        echo "Dry run requires Python 3.10.0+ and will not install it." >&2
        echo "Install a supported Python explicitly, then rerun this command." >&2
        exit 127
    fi
    case "$(uname -s)" in
        Darwin)
            if ! command -v brew >/dev/null 2>&1; then
                echo "Python 3.10.0+ is missing and Homebrew is unavailable." >&2
                echo "Install Python 3.10.0+ explicitly, then rerun this command." >&2
                exit 127
            fi
            brew install python@3.14
            python_command="$(brew --prefix python@3.14)/libexec/bin/python3"
            ;;
        Linux)
            if ! command -v apt-get >/dev/null 2>&1; then
                echo "Python 3.10.0+ is missing and this Linux host has no apt-get." >&2
                echo "Install Python 3.10.0+ with the host package manager, then rerun." >&2
                exit 127
            fi
            privilege=()
            if [[ "$(id -u)" -ne 0 ]]; then
                if ! command -v sudo >/dev/null 2>&1; then
                    echo "Python bootstrap requires root or sudo on this host." >&2
                    exit 127
                fi
                privilege=(sudo)
            fi
            "${privilege[@]}" apt-get update
            "${privilege[@]}" apt-get install -y python3
            python_command="$(command -v python3 || true)"
            ;;
        *)
            echo "Automatic Python bootstrap supports macOS and apt-based Linux only." >&2
            echo "Install Python 3.10.0+ explicitly, then rerun this command." >&2
            exit 127
            ;;
    esac
fi

if [[ -z "$python_command" ]] ||
   ! "$python_command" -c 'import sys; raise SystemExit(sys.version_info < (3, 10, 0))'; then
    echo "Bootstrap requires Python 3.10.0 or newer." >&2
    [[ -z "$python_command" ]] || "$python_command" --version >&2
    exit 127
fi

exec "$python_command" scripts/aux_tools.py bootstrap "$@"
