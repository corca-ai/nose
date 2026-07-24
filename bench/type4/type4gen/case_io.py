"""Stable identifiers, source records, and surface-pair selection."""

from __future__ import annotations

import hashlib
from pathlib import Path

from type4gen.model import Surface, Variant


def stable_id(*parts: str) -> str:
    h = hashlib.sha256()
    for p in parts:
        h.update(p.encode())
        h.update(b"\0")
    return h.hexdigest()[:16]


def rel_source_path(case_id: str, side: str, surface: Surface) -> Path:
    return Path("sources") / case_id / f"{side}.{surface.extension}"


def source_record(surface: Surface, variant: Variant, path: Path) -> dict:
    return {
        "language": surface.language,
        "surface": surface.key,
        "representation": variant.representation,
        "path": path.as_posix(),
        "entrypoint": variant.entrypoint,
        "start_line": variant.start_line,
        "end_line": variant.start_line + len(variant.source.rstrip("\n").splitlines()) - 1,
    }


def write_source(out_dir: Path, rel_path: Path, source: str) -> None:
    full = out_dir / rel_path
    full.parent.mkdir(parents=True, exist_ok=True)
    full.write_text(source)


def cross_pairs(surfaces: list[Surface], mode: str) -> list[tuple[Surface, Surface]]:
    if mode == "none":
        return []
    if mode == "ring":
        return [(surfaces[i], surfaces[(i + 1) % len(surfaces)]) for i in range(len(surfaces))]
    if mode == "all":
        return [(a, b) for i, a in enumerate(surfaces) for b in surfaces[i + 1 :]]
    raise ValueError(f"unknown cross mode: {mode}")
