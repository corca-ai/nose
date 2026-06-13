# The value model treats an indexed store `a[i] = v` as an opaque ordered effect and does
# NOT update readable state, so a later `a[i]` read re-derives the PRE-write value (fields
# are versioned via `field_env`; array elements are not). So `swap` and `clobber` below
# compute the same effect trace and share an exact-value-graph fingerprint — a false merge:
# swap(a,0,1) on [1,2] gives [2,1], clobber gives [2,2]. OPEN, ORACLE-BLIND: the interpreter
# shares the no-mutation model, so `nose verify` cannot witness it (no battery row
# distinguishes them) — the same category as float_assoc.py. Closing it needs in-place
# element-mutation modeling in the value graph AND the interpreter. See
# docs/oracle-value-model.md §7.3 (coevo series 9).
def swap(a, i, j):
    t = a[i]
    a[i] = a[j]
    a[j] = t


def clobber(a, i, j):
    a[i] = a[j]
    a[j] = a[i]
