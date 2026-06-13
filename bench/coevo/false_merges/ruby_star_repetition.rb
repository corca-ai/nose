# Ruby `*` is string/array REPETITION and asymmetric: `"ab" * 3` → "ababab" but
# `3 * "ab"` raises (Integer#* rejects a String). The algebra pass folded the constant
# to the chain end and the value graph sorted commutative operands by hash, so the two
# orders false-merged into one exact-value-graph family. Series 9; FIXED — `*` commute is
# now gated (Ruby + possible-string operand stays ordered). LATENT: the pinned corpus has
# no such pair, so `nose verify bench/repos` stayed green.
def rep_str_first
  "ab" * 3
end

def rep_int_first
  3 * "ab"
end
