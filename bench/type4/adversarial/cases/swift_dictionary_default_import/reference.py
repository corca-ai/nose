def dictionaryDefaultReference(
    lookup: dict[str, int], key: str, fallback: int
) -> int:
    return lookup.get(key, fallback)
