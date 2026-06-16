def handle(records, threshold):
    out = []
    total = 0
    for rec in records:
        parsed = parse(rec)
        score = evaluate(parsed)
        if score > threshold:
            total = total + score
            out.append(parsed)
        else:
            log_skip(rec)
    return summarize(out, total)
