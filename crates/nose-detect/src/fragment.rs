//! Substrate for exact sub-function semantic fragments.
//!
//! Exact fragment extraction historically grew as a set of standalone predicates in
//! [`crate::units`]: each accepted shape (direct return, conditional guard, append
//! effect, Java `this.field` body, …) was a boolean branch, and the *reason* a
//! fragment was accepted lived only in the shape of the predicate that matched it.
//!
//! This module is the first-class classification those predicates lower into. Every
//! accepted fragment root carries a [`FragmentKind`], a stable [`reason_code`], and a
//! set of [`ProofFacts`] describing what was proven at acceptance time. The predicates
//! in [`crate::units`] remain the recognizers; they now return a `FragmentKind` instead
//! of a bare `bool`, so downstream code (reporting, ranking, the fragment oracle) can
//! reason about *why* a fragment is exact-safe without re-reading the predicate matrix.
//!
//! Step 1 of issue #33 is deliberately behavior-invariant: each [`FragmentKind`] variant
//! corresponds 1:1 to a predicate branch that previously returned `true`, and the set of
//! accepted roots is unchanged. Later steps re-express these recognizers through an
//! explicit fragment contract and an independent behavior oracle.
//!
//! [`reason_code`]: FragmentKind::reason_code

mod contract;
mod oracle;

pub use contract::{FragmentContract, Place};
pub use oracle::{fragment_behavior, free_input_cids, synthesize_wrapper};

/// The shape of an accepted exact sub-function fragment.
///
/// Each variant is the classification of one recognizer branch in
/// [`crate::units::exact_statement_fragment_root`]. The discriminant is the *why*: a
/// `DirectReturn` fragment is an exact-safe `return <expr>` lifted out of a larger body,
/// a `LoopEffect` fragment is a `for-each` whose body is a single proven append, and so
/// on. Two fragments with the same kind were accepted by the same proof argument.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FragmentKind {
    /// `return <computed expr>` — a value sink whose returned expression is exact-safe.
    DirectReturn,
    /// `throw <computed expr>` — a control sink whose thrown expression is exact-safe.
    DirectThrow,
    /// `target[key] = value` — a non-overloadable index-assignment effect (C/Go/Java).
    IndexAssignEffect,
    /// `this.field = value` — a Java fixed-receiver self-field write.
    SelfFieldAssign,
    /// A single expression statement evaluated for its proven side effect.
    ExprEffect,
    /// An `if`/`else` whose non-empty branches are themselves exact exit/effect shapes.
    ConditionalGuard,
    /// A `for-each` loop whose body is a single iteration-dependent append effect.
    LoopEffect,
    /// A Java method body of `this.field = …` writes (plus optional `return this`).
    SelfFieldBody,
}

impl FragmentKind {
    /// A stable, user-facing kebab-case identifier for this fragment shape.
    ///
    /// Reason codes are part of the detector's external contract (issue #11): they name
    /// *why* a fragment is an exact semantic clone. They must stay stable across releases,
    /// so changing one is a breaking change to consumers that key on it.
    pub fn reason_code(self) -> &'static str {
        match self {
            FragmentKind::DirectReturn => "exact-direct-return",
            FragmentKind::DirectThrow => "exact-direct-throw",
            FragmentKind::IndexAssignEffect => "exact-index-assign-effect",
            FragmentKind::SelfFieldAssign => "exact-self-field-assign",
            FragmentKind::ExprEffect => "exact-expr-effect",
            FragmentKind::ConditionalGuard => "exact-conditional-guard",
            FragmentKind::LoopEffect => "exact-loop-effect",
            FragmentKind::SelfFieldBody => "exact-self-field-body",
        }
    }

    /// The control sink this fragment terminates in, when it is a pure exit shape.
    ///
    /// Effect and body fragments fall through to normal completion ([`Exit::Normal`]);
    /// the dedicated control sinks (`return`/`throw`) report themselves. This is the
    /// seed of the contract's exit set — later steps widen it to break/continue once
    /// those shapes are admitted.
    pub fn primary_exit(self) -> Exit {
        match self {
            FragmentKind::DirectReturn => Exit::Return,
            FragmentKind::DirectThrow => Exit::Throw,
            FragmentKind::IndexAssignEffect
            | FragmentKind::SelfFieldAssign
            | FragmentKind::ExprEffect
            | FragmentKind::ConditionalGuard
            | FragmentKind::LoopEffect
            | FragmentKind::SelfFieldBody => Exit::Normal,
        }
    }
}

/// A fragment's terminal control behavior.
///
/// Kept intentionally small for Step 1 — only the exits the current recognizers can
/// produce. `break`/`continue` are reserved for when loop-body fragment windows open.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Exit {
    /// Falls off the end of the fragment into the enclosing body.
    Normal,
    /// Returns a value out of the enclosing function.
    Return,
    /// Throws/raises out of the enclosing function.
    Throw,
}

/// What the recognizer proved about a fragment at acceptance time.
///
/// These are the invariants the exact gate already established — recorded explicitly so
/// later steps (the contract lowering and the behavior oracle) can consume them instead
/// of re-deriving them from the IL. Step 1 records only facts that are guaranteed true by
/// construction of the accepting predicate; the set grows as the contract model lands.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProofFacts {
    /// No preceding statement in the enclosing block aliases or mutates the fragment's
    /// free inputs, so the fragment's meaning does not depend on hidden prior state.
    ///
    /// True by construction for the top-level statement recognizers (they pass through
    /// `top_level_statement_fragment_context_safe`). The Java self-field *body* shape
    /// proves self-containment through its own receiver-fixed analysis rather than the
    /// shared context gate, and records `false` here to mark that distinction.
    pub context_safe: bool,
    /// The fragment's terminal control behavior (see [`Exit`]).
    pub exit: Exit,
}

impl ProofFacts {
    /// Proof facts for a `kind` accepted through the shared top-level context gate.
    pub fn context_gated(kind: FragmentKind) -> Self {
        ProofFacts {
            context_safe: true,
            exit: kind.primary_exit(),
        }
    }

    /// Proof facts for the Java self-field body shape, which establishes
    /// self-containment through its fixed `this` receiver rather than the shared gate.
    pub fn self_field_body() -> Self {
        ProofFacts {
            context_safe: false,
            exit: Exit::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_codes_are_distinct_and_kebab() {
        let kinds = [
            FragmentKind::DirectReturn,
            FragmentKind::DirectThrow,
            FragmentKind::IndexAssignEffect,
            FragmentKind::SelfFieldAssign,
            FragmentKind::ExprEffect,
            FragmentKind::ConditionalGuard,
            FragmentKind::LoopEffect,
            FragmentKind::SelfFieldBody,
        ];
        let mut codes: Vec<&str> = kinds.iter().map(|k| k.reason_code()).collect();
        let total = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), total, "reason codes must be unique");
        for code in codes {
            assert!(
                code.starts_with("exact-")
                    && code.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "reason code `{code}` must be kebab-case and exact-prefixed"
            );
        }
    }

    #[test]
    fn primary_exit_matches_sink_kinds() {
        assert_eq!(FragmentKind::DirectReturn.primary_exit(), Exit::Return);
        assert_eq!(FragmentKind::DirectThrow.primary_exit(), Exit::Throw);
        assert_eq!(FragmentKind::LoopEffect.primary_exit(), Exit::Normal);
    }
}
