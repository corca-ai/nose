import formal.lib.Effects

namespace NoseFragmentEffectPlace

open NoseFormal.Effects

theorem append_carries_no_receiver_obligation (place : Option Place) :
    siteProven { effect := Effect.append, place := place } := by
  exact append_site_proven place

theorem index_carries_no_receiver_obligation (place : Option Place) :
    siteProven { effect := Effect.indexWrite, place := place } := by
  exact index_site_proven place

theorem field_write_requires_proven_place (place : Option Place) :
    siteProven { effect := Effect.fieldWrite, place := place } <->
      exists proven, place = some proven /\ exactSafe proven := by
  exact field_site_proven_iff place

theorem unknown_receiver_is_rejected :
    Not (siteProven { effect := Effect.fieldWrite, place := some Place.unknown }) := by
  exact field_unknown_not_proven

theorem nested_unknown_receiver_is_rejected (field key : Nat) :
    Not (exactSafe (Place.field (Place.index Place.unknown key) field)) := by
  exact nested_unknown_not_safe field key

end NoseFragmentEffectPlace
