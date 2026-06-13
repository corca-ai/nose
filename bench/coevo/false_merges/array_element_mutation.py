# CLOSED (#337). `swap(a,0,1)` on [1,2] gives [2,1]; `clobber` gives [2,2] — different
# behavior. They once shared an exact-value-graph fingerprint because an indexed store
# `a[i] = v` was an opaque effect that did not update readable state, so a later `a[i]` read
# re-derived the PRE-write value. Now the value graph FORWARDS a post-write read of `base[index]`
# to the written value (`value_graph/index_state.rs`) and the interpreter mutates the array in
# place (`interp.rs` `bind` for `Index`), so the two are split AND oracle-witnessed. Kept as a
# guard; the permanent tests are `array_element_swap_does_not_merge_with_clobber` (equivalence)
# and `index_store_is_observed_by_later_read` (interp). See docs/oracle-value-model.md §7.3.
def swap(a, i, j):
    t = a[i]
    a[i] = a[j]
    a[j] = t


def clobber(a, i, j):
    a[i] = a[j]
    a[j] = a[i]
