/- Commutative Boolean core of CSS query canonicalization. -/

namespace NoseCssQuery

def conjunction (left right : Bool) : Bool := left && right
def alternatives (left right : Bool) : Bool := left || right

theorem conjunction_terms_commute (left right : Bool) :
    conjunction left right = conjunction right left := by
  cases left <;> cases right <;> rfl

theorem query_alternatives_commute (left right : Bool) :
    alternatives left right = alternatives right left := by
  cases left <;> cases right <;> rfl

end NoseCssQuery
