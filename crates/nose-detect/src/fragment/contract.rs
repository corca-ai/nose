//! The exact-fragment contract: the explicit, recognizer-independent description of a
//! sub-function fragment that the behavior oracle consumes.
//!
//! Issue #33 step 3. A [`FragmentContract`] is what a shape recognizer *produces* once it
//! accepts a fragment: the free inputs the fragment reads, the control exit it terminates
//! in, and the source node to lower. It deliberately carries only what the oracle needs to
//! synthesize a runnable wrapper (see [`super::oracle`]) — if a contract cannot be lowered
//! into a wrapper, the contract is underspecified, which is exactly the soundness property
//! we want to force.
//!
//! A contract describes a fragment as an ordered sequence of observable effects (possibly
//! empty, for a pure value/control sink) plus the control exit it terminates in. A
//! single-statement sink (direct return/throw) carries no effects; a single-statement write
//! carries one; a multi-statement body (a conditional branch, a loop body, an ordered effect
//! sequence) carries its effects in execution order. The [`Place`] receiver-identity model
//! is fail-closed so receiver-bearing effects can only be admitted when proven.
//!
//! proof-obligation: detect.fragment.effect_place

use super::{Exit, FragmentKind};
use nose_il::NodeId;

/// One observable effect in a fragment body, paired with its write-target identity.
///
/// The [`place`](Self::place) is `Some` only when the effect's soundness depends on knowing
/// *whose* state it touches — i.e. [`Effect::requires_proven_place`]. Append/index writes are
/// observable in the interpreter's effect trace and so carry no receiver-identity obligation;
/// their `place` is `None` even though they do mutate a collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectSite {
    /// How this effect is observed (see [`Effect`]).
    pub effect: Effect,
    /// The proven write target, for receiver-bearing effects; `None` otherwise.
    pub place: Option<Place>,
}

impl EffectSite {
    /// An effect whose observability carries no receiver-identity obligation (append/index).
    ///
    /// The effect/place split is an invariant of the model, not just a soundness check: a
    /// receiver-bearing effect (a field write) must record its [`Place`] via [`Self::at`], and
    /// an observable effect must not carry one. Enforced at construction so a malformed
    /// contract is caught the moment it is built, before [`FragmentContract::writes_proven`].
    pub fn observable(effect: Effect) -> Self {
        debug_assert!(
            !effect.requires_proven_place(),
            "observable() is for effects with no receiver obligation, not {effect:?}"
        );
        EffectSite {
            effect,
            place: None,
        }
    }

    /// A receiver-bearing effect (a field write) over a resolved [`Place`].
    pub fn at(effect: Effect, place: Place) -> Self {
        debug_assert!(
            effect.requires_proven_place(),
            "at() is for receiver-bearing effects, not {effect:?}"
        );
        EffectSite {
            effect,
            place: Some(place),
        }
    }
}

/// A first-class description of one exact sub-function fragment.
///
/// The contract is recognizer-independent: two fragments with the same inputs, exit, and
/// ordered effects are interchangeable to the oracle regardless of which predicate matched
/// them. The [`root`](Self::root) points at the fragment statement (or block) in the *source*
/// IL; lowering deep-copies that subtree into a synthetic wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FragmentContract {
    /// Which recognizer shape produced this contract.
    pub kind: FragmentKind,
    /// The fragment statement (or block) in the source IL, lowered by the oracle.
    pub root: NodeId,
    /// Free canonical ids the fragment reads, in canonical (ascending) order. These become
    /// the synthetic wrapper's parameters, bound positionally from the input battery.
    pub inputs: Vec<u32>,
    /// The control sink the fragment terminates in.
    pub exit: Exit,
    /// The fragment's observable effects, in execution order. Empty for pure value/control
    /// sinks; one entry for a single-effect write; several for an ordered multi-effect body.
    pub effects: Vec<EffectSite>,
}

impl FragmentContract {
    /// The arity of the synthesized wrapper — one parameter per free input.
    pub fn arity(&self) -> usize {
        self.inputs.len()
    }

    /// A pure value/control-sink contract (direct return/throw): no observable effects.
    pub fn value_sink(kind: FragmentKind, root: NodeId, inputs: Vec<u32>, exit: Exit) -> Self {
        FragmentContract {
            kind,
            root,
            inputs,
            exit,
            effects: Vec::new(),
        }
    }

    /// A single-effect contract (one append/index/field/other write), normal exit.
    pub fn single_effect(
        kind: FragmentKind,
        root: NodeId,
        inputs: Vec<u32>,
        site: EffectSite,
    ) -> Self {
        FragmentContract {
            kind,
            root,
            inputs,
            exit: Exit::Normal,
            effects: vec![site],
        }
    }

    /// A multi-effect contract: an ordered sequence of effects over a (block) body.
    pub fn ordered_effects(
        kind: FragmentKind,
        root: NodeId,
        inputs: Vec<u32>,
        exit: Exit,
        effects: Vec<EffectSite>,
    ) -> Self {
        FragmentContract {
            kind,
            root,
            inputs,
            exit,
            effects,
        }
    }

    /// Whether every receiver-bearing effect has a proven, exact-safe [`Place`].
    ///
    /// Fail-closed: a [`Effect::FieldWrite`] with no place, or a place rooted at
    /// [`Place::Unknown`], is not proven. Effects that carry no receiver obligation
    /// ([`Effect::Append`]/[`Effect::IndexWrite`]) never block this. This is the soundness
    /// predicate receiver-bearing shapes gate on before the contract is admitted.
    pub fn writes_proven(&self) -> bool {
        self.effects.iter().all(|site| {
            !site.effect.requires_proven_place()
                || site.place.as_ref().is_some_and(Place::is_exact_safe)
        })
    }
}

/// The observable-effect algebra for effect-bearing fragments.
///
/// Issue #33 asked to "extend the effect algebra rather than treating every mutation as
/// append-like". Each variant names how the effect is observed — which is exactly what
/// determines its soundness obligations in the behavior oracle:
///
/// - [`Effect::Append`] and [`Effect::IndexWrite`] are recorded in the interpreter's
///   *ordered effect trace*: the written key/value is observable, so two fragments that
///   write different keys or values diverge in [`Behavior`](nose_normalize::Behavior)
///   without needing receiver-identity proof.
/// - [`Effect::FieldWrite`] is recorded in the interpreter's *field-state map*, keyed by
///   field name only — the receiver identity is **not** observed. A field write is
///   therefore exact-safe only when its [`Place`] receiver is proven (fail-closed); this is
///   why only a fixed `this` receiver is admitted today.
/// - [`Effect::Other`] is any other proven single effect (e.g. a generic effectful call
///   statement) whose observability the oracle establishes by running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// An ordered append/push to a collection (free monoid over appended values).
    Append,
    /// An index/key assignment `target[key] = value`; key and value are observable.
    IndexWrite,
    /// A field assignment `receiver.field = value`; final per-field state, receiver
    /// identity unobserved — requires a proven [`Place`].
    FieldWrite,
    /// Any other single proven observable effect.
    Other,
}

impl Effect {
    /// Whether soundness for this effect requires the write target's [`Place`] to be a
    /// proven (exact-safe) receiver. Only field writes do: the interpreter does not observe
    /// the receiver of a field write, so an unproven receiver could falsely merge.
    pub fn requires_proven_place(self) -> bool {
        matches!(self, Effect::FieldWrite)
    }
}

/// A write target's proven identity — the receiver-identity model heap-effect fragments
/// migrate onto in step 6.
///
/// It is defined now, ahead of the effect-algebra work, so the substrate has a single
/// fail-closed answer to "whose state does this write touch?". The cardinal rule is that
/// [`Place::Unknown`] is the **default**: any receiver that does not resolve to a proven
/// place is `Unknown` and therefore not exact-safe. A fragment that writes through an
/// `Unknown` place must be rejected, never merged.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// A fixed `self`/`this` receiver (currently Java `this`).
    This,
    /// A parameter's identity, keyed by canonical id.
    Param(u32),
    /// A locally allocated object/collection, keyed by its allocation site's canonical id.
    LocalAlloc(u32),
    /// A field path off another place: `<base>.<field>`, field keyed by its name hash.
    Field(Box<Place>, u64),
    /// An index/key path off another place: `<base>[<key>]`, key keyed by a stable hash.
    Index(Box<Place>, u64),
    /// An unproven receiver. The fail-closed default — writes here are never exact-safe.
    Unknown,
}

impl Place {
    /// Whether this place's identity is proven well enough to support an exact effect.
    /// Fail-closed: anything reaching an [`Place::Unknown`] base is rejected.
    pub fn is_exact_safe(&self) -> bool {
        match self {
            Place::This | Place::Param(_) | Place::LocalAlloc(_) => true,
            Place::Field(base, _) | Place::Index(base, _) => base.is_exact_safe(),
            Place::Unknown => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_place_is_fail_closed() {
        assert!(!Place::Unknown.is_exact_safe());
        // An otherwise-proven path rooted at Unknown stays unsafe.
        let nested = Place::Field(Box::new(Place::Index(Box::new(Place::Unknown), 7)), 3);
        assert!(!nested.is_exact_safe());
        // A proven receiver with a field/index path is safe.
        assert!(Place::Field(Box::new(Place::This), 3).is_exact_safe());
        assert!(Place::Index(Box::new(Place::Param(0)), 9).is_exact_safe());
    }

    use super::super::FragmentKind;
    use nose_il::NodeId;

    #[test]
    fn writes_proven_gates_only_receiver_bearing_effects() {
        let root = NodeId(0);
        // No effects: trivially proven (a pure sink).
        assert!(FragmentContract::value_sink(
            FragmentKind::DirectReturn,
            root,
            vec![],
            Exit::Return
        )
        .writes_proven());

        // Append/index writes carry no receiver obligation — proven regardless of place.
        let appends = FragmentContract::ordered_effects(
            FragmentKind::ExprEffect,
            root,
            vec![],
            Exit::Normal,
            vec![
                EffectSite::observable(Effect::Append),
                EffectSite::observable(Effect::IndexWrite),
            ],
        );
        assert!(appends.writes_proven());

        // A field write over a proven `This` place is admitted; over Unknown it is not.
        let proven = FragmentContract::single_effect(
            FragmentKind::SelfFieldAssign,
            root,
            vec![],
            EffectSite::at(Effect::FieldWrite, Place::Field(Box::new(Place::This), 7)),
        );
        assert!(proven.writes_proven());

        let unproven = FragmentContract::single_effect(
            FragmentKind::SelfFieldAssign,
            root,
            vec![],
            EffectSite::at(
                Effect::FieldWrite,
                Place::Field(Box::new(Place::Unknown), 7),
            ),
        );
        assert!(
            !unproven.writes_proven(),
            "field write through Unknown is fail-closed"
        );

        // A field write with no resolved place at all is fail-closed too.
        let missing = FragmentContract::ordered_effects(
            FragmentKind::SelfFieldBody,
            root,
            vec![],
            Exit::Normal,
            vec![EffectSite {
                effect: Effect::FieldWrite,
                place: None,
            }],
        );
        assert!(!missing.writes_proven());
    }
}
