/- CSS box-shorthand expansion laws. -/

namespace NoseCssBox

structure Box (α : Type) where
  top : α
  right : α
  bottom : α
  left : α
  deriving DecidableEq

inductive Spelling (α : Type) where
  | one : α → Spelling α
  | two : α → α → Spelling α
  | three : α → α → α → Spelling α
  | four : α → α → α → α → Spelling α

def expand : Spelling α → Box α
  | .one a => ⟨a, a, a, a⟩
  | .two a b => ⟨a, b, a, b⟩
  | .three a b c => ⟨a, b, c, b⟩
  | .four a b c d => ⟨a, b, c, d⟩

theorem four_equal_to_one (a : α) :
    expand (.four a a a a) = expand (.one a) := by
  rfl

theorem four_pair_to_two (a b : α) :
    expand (.four a b a b) = expand (.two a b) := by
  rfl

theorem four_sides_to_three (a b c : α) :
    expand (.four a b c b) = expand (.three a b c) := by
  rfl

structure Axes (α : Type) where
  block : α
  inline : α
  deriving DecidableEq

inductive AxesSpelling (α : Type) where
  | one : α → AxesSpelling α
  | two : α → α → AxesSpelling α

def expandAxes : AxesSpelling α → Axes α
  | .one a => ⟨a, a⟩
  | .two block inline => ⟨block, inline⟩

theorem two_axes_equal_to_one (a : α) :
    expandAxes (.two a a) = expandAxes (.one a) := by
  rfl

end NoseCssBox
