function loadAll(ids, opts) {
    const out = [];
    let total = 0;
    for (const id of ids) {
        const item = fetchItem(id);
        const w = weigh(item, opts);
        if (w > opts.min) {
            total += w;
            out.push(item);
        }
    }
    return finalize(out, total);
}
