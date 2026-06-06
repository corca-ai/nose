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
//! This is the minimal contract: it models the direct-return shape (inputs + a value/throw
//! exit). Heap writes, effect algebra, and [`Place`] receiver identity are layered on in
//! later steps; the placeholder [`Place`] enum below fixes the fail-closed default now so
//! receiver-bearing shapes have a home to migrate into.

use super::{Exit, FragmentKind};
use nose_il::NodeId;

/// A first-class description of one exact sub-function fragment.
///
/// The contract is recognizer-independent: two fragments with the same inputs, exit, and
/// (eventually) effects are interchangeable to the oracle regardless of which predicate
/// matched them. The [`root`](Self::root) points at the fragment statement in the *source*
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
}

impl FragmentContract {
    /// The arity of the synthesized wrapper — one parameter per free input.
    pub fn arity(&self) -> usize {
        self.inputs.len()
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
}
