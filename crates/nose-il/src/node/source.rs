use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceFactKind {
    Operator(SourceOperatorKind),
    Cast(SourceCastKind),
    Call(SourceCallKind),
    Protocol(SourceProtocolKind),
    Literal(SourceLiteralKind),
    Comprehension(SourceComprehensionKind),
    Range(SourceRangeKind),
    Pattern(SourcePatternKind),
    Binding(SourceBindingKind),
}

/// Source facts about how a definition BINDS, not what its body computes. Consumers that
/// treat a `def`'s body as the NAME's content (call-target evidence, content-keyed
/// seeding, inlining) must see these and fail closed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceBindingKind {
    /// A decorated definition's runtime binding is `decorator(f)`, not `f` — lowering
    /// keeps only the inner body (sound for unit-shape analysis) (coevo series 6, S2-A).
    DecoratedDefinition,
    /// An assignment that REBINDS a module-scope name from inside another scope — a
    /// Python `global name; name = ...`. The frontend drops `global`/`nonlocal`, so this
    /// fact (anchored on the assignment, identifying the binding by its target name) is
    /// the only signal that `name`'s runtime binding is no longer its `def` body. Without
    /// it a non-top-level `name = x` is indistinguishable from a local declaration, which
    /// is why the series-6 reassigned-anywhere predicate over-fired (#302).
    ModuleRebind,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceCastKind {
    CUnsigned32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceOperatorKind {
    StrictEquality,
    StrictInequality,
    LooseEquality,
    LooseInequality,
    ValueEquality,
    ValueInequality,
    IdentityEquality,
    IdentityInequality,
    TypeMembership,
    Typeof,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceCallKind {
    Construct,
    MacroInvocation,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceProtocolKind {
    Await,
    AsyncBlock,
    AsyncFunction,
    AsyncContext,
    AsyncIteration,
    ChannelReceive,
    ChannelSelect,
    ChannelSelectCase,
    ChannelSelectDefault,
    ChannelSend,
    Defer,
    GoRoutine,
    TaskSpawn,
    TryPropagation,
    Yield,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceLiteralKind {
    Regex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceComprehensionKind {
    PythonDictComprehension,
    PythonGeneratorExpression,
    PythonListComprehension,
    PythonSetComprehension,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourceRangeKind {
    RustHalfOpenRangeExpression,
    RustInclusiveRangeExpression,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SourcePatternKind {
    RustTupleStructSingleWildcardPattern,
}
