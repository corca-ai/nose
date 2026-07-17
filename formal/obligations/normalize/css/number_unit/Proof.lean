/- Parsed numeric spelling and zero-length unit normalization. -/

namespace NoseCssNumberUnit

inductive Dimension where
  | number
  | length
  | percentage
  | angle
  | time
  deriving DecidableEq

structure Scalar where
  numerator : Int
  decimalPlaces : Nat
  dimension : Dimension
  deriving DecidableEq

inductive Spelling where
  | canonical : Scalar → Spelling
  | leadingPlus : Scalar → Spelling
  | trailingZeros : Scalar → Spelling
  | zeroLength : Nat → Spelling

def denote : Spelling → Scalar
  | .canonical value => value
  | .leadingPlus value => value
  | .trailingZeros value => value
  | .zeroLength _ => ⟨0, 0, .length⟩

def canonicalize : Spelling → Spelling
  | spelling => .canonical (denote spelling)

theorem canonicalize_preserves_scalar (spelling : Spelling) :
    denote (canonicalize spelling) = denote spelling := by
  cases spelling <;> rfl

theorem zero_length_units_agree (leftUnit rightUnit : Nat) :
    denote (.zeroLength leftUnit) = denote (.zeroLength rightUnit) := by
  rfl

end NoseCssNumberUnit
