"""Stable, exclusively owned workspaces for base-view output comparisons."""
from contextlib import contextmanager
import os
from pathlib import Path
import re
import subprocess
import tempfile

DEFAULT_ROOT = Path(__file__).resolve().parents[1] / "target/query-regression-worktrees"


def git(repo: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, check=False
    )
    if result.returncode:
        raise SystemExit(f"{repo}: git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


@contextmanager
def detached_worktree(repo: Path, commit: str, root: Path | None = None):
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise SystemExit("base-view worktree requires an exact commit SHA")
    parent = (root or DEFAULT_ROOT).resolve() / repo.name
    parent.mkdir(parents=True, exist_ok=True)
    worktree = parent / commit
    reservation = parent / f"{commit}.lock"
    try:
        handle = reservation.open("x", encoding="utf-8")
    except FileExistsError as error:
        raise SystemExit(f"base-view worktree is already reserved: {reservation}") from error
    created = False
    try:
        with handle:
            handle.write(f"pid={os.getpid()}\nrepo={repo.resolve()}\n")
        if worktree.exists() or worktree.is_symlink():
            raise SystemExit(f"refusing to replace an existing base-view workspace: {worktree}")
        git(repo, "worktree", "add", "--detach", "--quiet", str(worktree), commit)
        created = True
        yield worktree
    finally:
        try:
            if created:
                git(repo, "worktree", "remove", "--force", str(worktree))
                git(repo, "worktree", "prune")
        finally:
            reservation.unlink()


def run_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="nose-worktree-test-") as directory:
        root = Path(directory)
        repo = root / "source"
        repo.mkdir()
        git(repo, "init", "--quiet")
        (repo / "value.txt").write_text("committed\n")
        git(repo, "add", "value.txt")
        git(repo, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
            "commit", "--quiet", "-m", "fixture")
        commit = git(repo, "rev-parse", "HEAD")
        (repo / "value.txt").write_text("uncommitted source\n")
        paths = []
        for _ in range(2):
            with detached_worktree(repo, commit, root / "workspaces") as path:
                paths.append(path)
                assert (path / "value.txt").read_text() == "committed\n"
                try:
                    with detached_worktree(repo, commit, root / "workspaces"):
                        raise AssertionError("concurrent reservation must fail")
                except SystemExit as error:
                    assert "already reserved" in str(error)
                (path / "value.txt").write_text("owned scratch\n")
            assert not path.exists()
        assert paths[0] == paths[1]
        assert (repo / "value.txt").read_text() == "uncommitted source\n"
        try:
            with detached_worktree(repo, commit, root / "workspaces") as path:
                raise ValueError("query failed")
        except ValueError:
            pass
        assert not path.exists()
        path.mkdir(parents=True)
        (path / "keep.txt").write_text("unowned\n")
        try:
            with detached_worktree(repo, commit, root / "workspaces"):
                raise AssertionError("existing workspace must be preserved")
        except SystemExit as error:
            assert "refusing to replace" in str(error)
        assert (path / "keep.txt").read_text() == "unowned\n"
        assert not path.with_name(path.name + ".lock").exists()
    print("stable base-view worktree self-test passed")
