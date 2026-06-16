def quantize(samples, bits):
    table = {}
    peak = 0
    for s in samples:
        q = round(s * bits)
        table[s] = q
        if q > peak:
            peak = q
    return table, peak
