function buildIndex(rows, cfg) {
    const idx = new Map();
    let n = 0;
    for (const row of rows) {
        const key = hashRow(row);
        idx.set(key, row);
        n = n + cfg.step;
    }
    return { idx, n };
}
