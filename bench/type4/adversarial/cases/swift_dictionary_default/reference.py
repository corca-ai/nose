def dictionaryDefaultReference(
    lookup: dict[str, int],
    key: str,
    fallback: int,
    otherLookup: dict[str, int],
    otherKey: str,
    otherDefault: int,
) -> int:
    return lookup.get(key, fallback)
