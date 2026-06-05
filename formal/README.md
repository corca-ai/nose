# Formal core — machine-checked soundness of value-graph canonicalization

nose's soundness contract (§AJ) is *fingerprint-equal ⟹ behavior-equal*. Until now that
was only **empirical** ("0 false merges on N repos", checked by `nose verify`). This
directory makes the core canonicalization rules **machine-checked** in Lean 4: each rule
is proven to preserve a denotational semantics of the IL, so it provably cannot change
behavior. The empirical `verify` oracle then serves as a differential cross-check (it
catches *lowering* gaps the formal model abstracts over).

This is the language-agnostic, rigorous layer; per-language lowering optimization rides on
top of it (see `docs/experiments.md` §AU/§AV and the phase plan).

## Proven

- **`Algebra.lean`** — the associative-commutative operand canonicalization
  (`value_graph.rs`: `flatten_into` + `operands.sort_by_key`). `canon_sound`: if two
  expressions' flattened `+`-leaves are a permutation (which the structural-hash sort
  guarantees), they have equal denotation on every environment — flatten-then-sort is
  denotation-preserving. `sub_eq_add_neg`: `a - b ≡ a + (-b)` over `Int` (the subtraction
  canonicalization). The commutativity premise is now *enforced* by the value graph's type
  inference (`+` sorts only on numeric operands).
- **`Control.lean`** — control-flow canons over a minimal statement/return semantics.
  `guard_clause`: `if c {return a}; return b ≡ if c {return a} else {return b}` (the
  guard-clause path narrowing); `guard_clause_cascade`; `dead_code_after_return`;
  `ternary_return`: `return (a if c else b) ≡ if c {return a} else {return b}` (the
  `emit_return` ternary-decomposition — composed with `guard_clause` it converges a nested
  ternary with an `elif` cascade).
- **`Functor.lean`** — the list-functor laws. `map_fusion`: `map g (map f xs) = map (g∘f) xs`
  (justifies `Elem(Map f c) → f(Elem c)`, the map-fusion peel); `map_id`; `filter_fusion`:
  `filter q (filter p xs) = filter (λx. p x ∧ q x) xs` (justifies representing `filter(p,c)`
  as `Hof(Map, [Elem c, p])` and fusing nested filters via `and_preds`, so a two-filter
  comprehension, a `.filter().filter()` chain, and the filtered builder loop all converge);
  `filter_length_eq_count`: `(filter p xs).length = Σ (p x ? 1 : 0)` (justifies folding
  `len([c for x in xs if p])` to the same count-reduce as `sum(1 for x in xs if p)`).
- **`Compare.lean`** — comparison-direction and negated-comparison canons over `Int` (the
  total order the interpreter evaluates). `gt_eq_lt_swap`: `a > b ≡ b < a` (the `Gt → Lt`+swap
  canon); `ge_eq_le_swap`: `a >= b ≡ b <= a`; `not_le_eq_gt`/`not_lt_eq_ge`: `!(a<=b) ≡ a>b`,
  `!(a<b) ≡ a>=b` (the `negate_cmp_code` complements); `not_eq_eq_ne`: `!(a==b) ≡ a!=b`.
- **`Algebra.lean`** also has `neg_add_distrib`: `-(a+b) = -a + -b` (the `Neg(Add)` push-in)
  and `distrib_sound`: `(x+y)*f = x*f + y*f` over `Int` (the `factor_distribute` rewrite
  `a*c + b*c → (a+b)*c`, gated on every leaf being proven `Num` since the string/list
  `*`-as-repetition monoid is not a ring).
- **`BoolReduce.lean`** — the `any`/`all` predicate reductions. OR/AND are commutative
  monoids (`or_comm`/`or_assoc`/`or_id`/`or_idem`, and the AND duals) so the seedless
  `REDUCE_ANY`/`REDUCE_ALL` fold is well-defined; `vany_iff`/`vall_iff` show they denote
  existence/universality, and `any_map` shows mapping-then-folding equals the cross-language
  per-element fold — so Python `any(p(x) for x in xs)`, JS `xs.some(p)`, and Rust
  `xs.iter().any(p)` converge to one node.
- **`MinMax.lean`** — the 2-way min/max idiom. `min_is_ternary`/`max_is_ternary` (the
  canonical Min/Max node IS `x if x<y else y` — definitional soundness of the recognition);
  `vmin_comm`/`vmax_comm` (justify the commutative MIN/MAX codes); `vmin_assoc`/`vmax_assoc`
  (justify the min/max selection *reduction*); `vmin_idem`.

## Check

```
for f in formal/*.lean; do ~/.elan/bin/lean "$f"; done   # exit 0 each = proofs hold, no `sorry`
```

## Next (roadmap)

Formalize the remaining canons against the same semantics: field-write last-write-wins
commutativity, the `Reduce`/fold normal form (selection reductions build on `MinMax.lean`'s
proven min/max comm+assoc), and the free-monoid (ordered concat) model. Each proof closes a
class of potential false merges by construction rather than by corpus coverage.
