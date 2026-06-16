function collect(keys, cache) {
    const res = {};
    let hits = 0;
    for (const k of keys) {
        const raw = lookup(k);
        const norm = normalize(raw);
        if (norm.ok) {
            res[k] = norm.value;
            hits += 1;
        }
    }
    return { res, hits };
}
