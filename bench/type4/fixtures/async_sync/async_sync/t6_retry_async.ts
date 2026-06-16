async function withRetry(tasks, limit) {
    const done = [];
    let attempts = 0;
    for (const t of tasks) {
        const r = await execute(t);
        const ok = await check(r);
        if (ok) {
            done.push(r);
        } else {
            attempts += 1;
        }
    }
    return { done, attempts };
}
