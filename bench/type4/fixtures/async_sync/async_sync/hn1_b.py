def tally(ballots, base):
    seen = {}
    running = base
    for b in ballots:
        w = weight(b)
        running = running - w * 2
        seen[b] = running
    return finalize(seen, running)
