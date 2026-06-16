function withRetry(tasks, limit) {
    const done = [];
    let attempts = 0;
    for (const t of tasks) {
        const r = execute(t);
        const ok = check(r);
        if (ok) {
            done.push(r);
        } else {
            attempts += 1;
        }
    }
    return { done, attempts };
}
