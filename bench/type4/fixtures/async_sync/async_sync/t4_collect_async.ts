async function collect(keys, cache) {
    const res = {};
    let hits = 0;
    for (const k of keys) {
        const raw = await lookup(k);
        const norm = await normalize(raw);
        if (norm.ok) {
            res[k] = norm.value;
            hits += 1;
        }
    }
    return { res, hits };
}
