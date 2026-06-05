//! Literal abstraction.
//!
//! Literal values are already abstracted to their [`nose_il::LitClass`] by the
//! frontends (numbers → `Int`/`Float`, strings → `Str`, etc.), which is all v1
//! needs. This module is the seam for an optional, flag-controlled pass that
//! *retains* small constants (e.g. `0`/`1`) as `Payload::LitInt` for tighter
//! matching — not yet enabled.
