"""Compare full query payloads while checking navigation's cache context."""

import copy
import json
import shlex
from pathlib import Path

NORMALIZER = "nose.query-cache-navigation/v1"


def comparable_output(payload: dict, cache: Path | None) -> bytes:
    """Only erase the exact cache argument from executable navigation commands."""
    def command(value: str) -> str:
        tokens = shlex.split(value)
        if tokens[:2] != ["nose", "query"]:
            return value
        positions = [i for i, token in enumerate(tokens) if token == "--cache-dir"]
        if len(positions) > 1:
            raise ValueError("duplicate cache argument in navigation")
        if cache is not None and payload.get("schema_version", 0) >= 10 and not positions:
            raise ValueError("navigation lost its query cache context")
        if positions:
            index = positions[0]
            if cache is None or tokens[index + 1:index + 2] != [str(cache)]:
                raise ValueError("navigation cache argument differs from query context")
            del tokens[index:index + 2]
        return shlex.join(tokens)

    def visit(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == "next" and isinstance(child, list):
                    value[key] = [command(item) if isinstance(item, str) else item for item in child]
                elif key == "actions" and isinstance(child, list):
                    for action in child:
                        if isinstance(action, dict) and isinstance(action.get("command"), str):
                            action["command"] = command(action["command"])
                else:
                    visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    result = copy.deepcopy(payload)
    visit(result)
    return json.dumps(result, sort_keys=True, separators=(",", ":")).encode()


def self_test() -> None:
    cache = Path("cache ' $ dir")
    clean = {"schema_version": 10, "families": [{"id": "abc"}], "next": ["nose query . id=abc"],
             "actions": [{"kind": "open-family", "command": "nose query . id=abc"}]}
    cached = copy.deepcopy(clean)
    cached["next"][0] += " --cache-dir " + shlex.quote(str(cache))
    cached["actions"][0]["command"] = cached["next"][0]
    assert comparable_output(clean, None) == comparable_output(cached, cache)
    changed = copy.deepcopy(cached)
    changed["families"][0]["id"] = "def"
    assert comparable_output(clean, None) != comparable_output(changed, cache)
    changed = copy.deepcopy(cached)
    changed["next"][0] += " scope=test"
    assert comparable_output(clean, None) != comparable_output(changed, cache)
    for wrong_cache in (None, Path("wrong")):
        try:
            comparable_output(cached, wrong_cache)
        except ValueError:
            pass
        else:
            raise AssertionError("wrong cache context accepted")
    assert clean["next"][0] == "nose query . id=abc"
