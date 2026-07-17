/- Parsed CSS color-spelling normalization. Parser/table correspondence is recorded
separately as an empirical runtime precondition in meta.toml. -/

namespace NoseCssColor

structure Rgba where
  red : Nat
  green : Nat
  blue : Nat
  alpha : Nat
  deriving DecidableEq

inductive Spelling where
  | canonical : Rgba → Spelling
  | shortHex : Rgba → Spelling
  | named : Rgba → Spelling
  | opaqueAlpha : Rgba → Spelling

def denote : Spelling → Rgba
  | .canonical value => value
  | .shortHex value => value
  | .named value => value
  | .opaqueAlpha value => value

def canonicalize : Spelling → Spelling
  | spelling => .canonical (denote spelling)

theorem canonicalize_preserves_rgba (spelling : Spelling) :
    denote (canonicalize spelling) = denote spelling := by
  cases spelling <;> rfl

theorem alias_spellings_agree (value : Rgba) :
    denote (.named value) = denote (.canonical value) := by
  rfl

end NoseCssColor
