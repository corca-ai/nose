async def handle(records, threshold):
    out = []
    total = 0
    for rec in records:
        parsed = await parse(rec)
        score = await evaluate(parsed)
        if score > threshold:
            total = total + score
            out.append(parsed)
        else:
            await log_skip(rec)
    return summarize(out, total)
