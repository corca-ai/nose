#!/usr/bin/env python3
"""Stable binary identity for performance comparisons.

Darwin's linker gives otherwise identical Mach-O binaries a fresh ``LC_UUID``
and ad-hoc code signature. The full-file digest remains useful for artifact
provenance, while the normalized code digest prevents those non-executable
bytes from masquerading as a code change.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
from pathlib import Path
import struct
import tempfile


FULL_FILE_ALGORITHM = "sha256/full-file-v1"
MACH_O_CODE_ALGORITHM = "sha256/mach-o-zero-uuid-signature-v1"
MH_MAGIC = 0xFEEDFACE
MH_MAGIC_64 = 0xFEEDFACF
LC_UUID = 0x1B
LC_CODE_SIGNATURE = 0x1D


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


@dataclass(frozen=True)
class BinaryIdentity:
    file_sha256: str
    code_sha256: str
    code_sha256_algorithm: str


def _normalized_mach_o(data: bytes) -> bytes | None:
    if len(data) < 28:
        return None
    magic = struct.unpack_from("<I", data)[0]
    if magic not in (MH_MAGIC, MH_MAGIC_64):
        return None

    header_size = 32 if magic == MH_MAGIC_64 else 28
    if len(data) < header_size:
        return None
    command_count = struct.unpack_from("<I", data, 16)[0]
    offset = header_size
    normalized = bytearray(data)
    found_identity_bytes = False

    for _ in range(command_count):
        if offset + 8 > len(data):
            return None
        command, command_size = struct.unpack_from("<II", data, offset)
        if command_size < 8 or offset + command_size > len(data):
            return None
        if command == LC_UUID and command_size >= 24:
            normalized[offset + 8 : offset + 24] = bytes(16)
            found_identity_bytes = True
        elif command == LC_CODE_SIGNATURE and command_size >= 16:
            data_offset, data_size = struct.unpack_from("<II", data, offset + 8)
            if data_offset + data_size > len(data):
                return None
            normalized[data_offset : data_offset + data_size] = bytes(data_size)
            found_identity_bytes = True
        offset += command_size

    return bytes(normalized) if found_identity_bytes else None


def binary_identity(path: Path) -> BinaryIdentity:
    data = path.read_bytes()
    file_sha256 = hashlib.sha256(data).hexdigest()
    normalized = _normalized_mach_o(data)
    if normalized is None:
        return BinaryIdentity(file_sha256, file_sha256, FULL_FILE_ALGORITHM)
    return BinaryIdentity(
        file_sha256,
        hashlib.sha256(normalized).hexdigest(),
        MACH_O_CODE_ALGORITHM,
    )


def run_self_test() -> None:
    header = struct.pack("<IiiIIIII", MH_MAGIC_64, 0, 0, 2, 2, 40, 0, 0)
    signature_offset = len(header) + 24 + 16 + 4

    def mach_o(uuid: bytes, signature: bytes, code: bytes = b"code") -> bytes:
        uuid_command = struct.pack("<II16s", LC_UUID, 24, uuid)
        signature_command = struct.pack(
            "<IIII", LC_CODE_SIGNATURE, 16, signature_offset, len(signature)
        )
        return header + uuid_command + signature_command + code + signature

    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        first = root / "first"
        second = root / "second"
        changed = root / "changed"
        plain = root / "plain"
        first.write_bytes(mach_o(bytes(range(16)), b"a" * 32))
        second.write_bytes(mach_o(bytes(reversed(range(16))), b"b" * 32))
        changed.write_bytes(mach_o(bytes(range(16)), b"a" * 32, b"edit"))
        plain.write_bytes(b"not a Mach-O")

        first_identity = binary_identity(first)
        second_identity = binary_identity(second)
        changed_identity = binary_identity(changed)
        plain_identity = binary_identity(plain)
        assert first_identity.file_sha256 != second_identity.file_sha256
        assert first_identity.code_sha256 == second_identity.code_sha256
        assert first_identity.code_sha256 != changed_identity.code_sha256
        assert first_identity.code_sha256_algorithm == MACH_O_CODE_ALGORITHM
        assert plain_identity.file_sha256 == plain_identity.code_sha256
        assert plain_identity.code_sha256_algorithm == FULL_FILE_ALGORITHM


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        parser.error("--self-test is required")
    run_self_test()
    print("binary identity self-test passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
