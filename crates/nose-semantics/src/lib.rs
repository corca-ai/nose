//! Semantic contracts for language and library facts used by exact matching.
//!
//! This crate is the first-party semantic-kernel facade. The initial migration is
//! deliberately behavior-preserving: it names the semantic assumptions that were
//! previously encoded as scattered `Lang` matches. Future pack loading should
//! extend this contract surface rather than letting packs mint fingerprints or
//! approve exact clone matches directly.

use nose_il::{
    stable_symbol_hash, Builtin, HoFKind, Il, Interner, Lang, LitClass, NodeId, NodeKind,
    ParamSemantic, Payload,
};

/// Stable pack id for the first-party language/stdlib contracts compiled into nose.
pub const FIRST_PARTY_PACK_ID: &str = "nose.first_party";

/// Channel a semantic fact or contract is safe to influence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelEligibility {
    SyntaxOnly,
    NearOnly,
    ExactEmpirical,
    ExactProven,
    FirstParty,
}

/// Kernel-facing domain evidence recovered from source facts, type inference, or future packs.
///
/// `nose-il::ParamSemantic` is the current frontend storage format for source-level parameter
/// annotations. Exact gates should consume this semantic-kernel vocabulary instead, so future
/// packs can provide equivalent evidence without becoming coupled to frontend syntax facts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DomainEvidence {
    Array,
    ByteArray,
    Collection,
    Integer,
    Map,
    Number,
    Option,
    Set,
    String,
}

impl DomainEvidence {
    pub fn from_param_semantic(semantic: ParamSemantic) -> Self {
        match semantic {
            ParamSemantic::Array => DomainEvidence::Array,
            ParamSemantic::ByteArray => DomainEvidence::ByteArray,
            ParamSemantic::Collection => DomainEvidence::Collection,
            ParamSemantic::Integer => DomainEvidence::Integer,
            ParamSemantic::Map => DomainEvidence::Map,
            ParamSemantic::Number => DomainEvidence::Number,
            ParamSemantic::Option => DomainEvidence::Option,
            ParamSemantic::Set => DomainEvidence::Set,
            ParamSemantic::String => DomainEvidence::String,
        }
    }

    pub fn is_array(self) -> bool {
        self == DomainEvidence::Array
    }

    pub fn is_byte_array(self) -> bool {
        self == DomainEvidence::ByteArray
    }

    pub fn is_collection_or_set(self) -> bool {
        matches!(self, DomainEvidence::Collection | DomainEvidence::Set)
    }

    pub fn is_array_or_collection(self) -> bool {
        matches!(self, DomainEvidence::Array | DomainEvidence::Collection)
    }

    pub fn is_array_collection_or_set(self) -> bool {
        matches!(
            self,
            DomainEvidence::Array | DomainEvidence::Collection | DomainEvidence::Set
        )
    }

    pub fn is_set(self) -> bool {
        self == DomainEvidence::Set
    }

    pub fn is_map(self) -> bool {
        self == DomainEvidence::Map
    }

    pub fn is_option(self) -> bool {
        self == DomainEvidence::Option
    }

    pub fn is_string(self) -> bool {
        self == DomainEvidence::String
    }

    pub fn is_integer(self) -> bool {
        self == DomainEvidence::Integer
    }

    pub fn is_integer_or_number(self) -> bool {
        matches!(self, DomainEvidence::Integer | DomainEvidence::Number)
    }
}

pub fn domain_evidence_from_param_semantic(semantic: ParamSemantic) -> DomainEvidence {
    DomainEvidence::from_param_semantic(semantic)
}

/// A first-party language profile. Keep this cheap and copyable; callers use it as a
/// named semantic boundary around currently-supported language behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LanguageProfile {
    lang: Lang,
}

pub fn semantics(lang: Lang) -> LanguageProfile {
    LanguageProfile { lang }
}

impl LanguageProfile {
    pub fn lang(self) -> Lang {
        self.lang
    }

    pub fn pack_id(self) -> &'static str {
        FIRST_PARTY_PACK_ID
    }

    pub fn eligibility(self) -> ChannelEligibility {
        ChannelEligibility::FirstParty
    }

    pub fn operators(self) -> OperatorSemantics {
        OperatorSemantics { lang: self.lang }
    }

    pub fn effects(self) -> EffectSemantics {
        EffectSemantics { lang: self.lang }
    }

    pub fn modules(self) -> ModuleSemantics {
        ModuleSemantics { lang: self.lang }
    }

    pub fn stdlib(self) -> StdlibSemantics {
        StdlibSemantics { lang: self.lang }
    }

    pub fn collections(self) -> CollectionSemantics {
        CollectionSemantics { lang: self.lang }
    }

    pub fn exact_fragments(self) -> FragmentSemantics {
        FragmentSemantics { lang: self.lang }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OperatorSemantics {
    lang: Lang,
}

impl OperatorSemantics {
    /// Source comparison operators are primitive total-order comparisons rather
    /// than receiver-overloadable/user-dispatched comparisons. This gates lattice
    /// comparison absorption rules.
    pub fn primitive_order_comparisons(self) -> bool {
        matches!(self.lang, Lang::C | Lang::Go | Lang::Java)
    }

    /// C unsigned byte/word packing contracts are currently first-party only for
    /// the C lowering, where explicit unsigned facts are recovered by the frontend.
    pub fn c_integer_byte_pack_contracts(self) -> bool {
        self.lang == Lang::C
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectSemantics {
    lang: Lang,
}

impl EffectSemantics {
    /// `target[key] = value` is modeled as a non-overloadable observable index
    /// write. Languages with user-dispatched index assignment must stay fail-closed
    /// unless a future pack emits a stronger receiver proof.
    pub fn non_overloadable_index_assignment(self) -> bool {
        matches!(self.lang, Lang::C | Lang::Go | Lang::Java)
    }

    /// Exact field-write fragments currently require Java's fixed `this.field`
    /// receiver proof.
    pub fn java_this_field_place(self) -> bool {
        self.lang == Lang::Java
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FragmentSemantics {
    lang: Lang,
}

impl FragmentSemantics {
    pub fn non_overloadable_index_assignment(self) -> bool {
        EffectSemantics { lang: self.lang }.non_overloadable_index_assignment()
    }

    pub fn java_this_field_place(self) -> bool {
        EffectSemantics { lang: self.lang }.java_this_field_place()
    }
}

/// Exact Java `this` receiver proof for first-party self-field fragments.
pub fn exact_java_this_var(il: &Il, interner: &Interner, node: NodeId) -> bool {
    semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        && il.kind(node) == NodeKind::Var
        && matches!(il.node(node).payload, Payload::Name(name) if interner.resolve(name) == "this")
}

/// Exact Java `this.field` place proof for receiver-free field-write fingerprints.
pub fn exact_java_this_field(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        || il.kind(node) != NodeKind::Field
    {
        return false;
    }
    if !matches!(il.node(node).payload, Payload::Name(_)) {
        return false;
    }
    il.children(node)
        .first()
        .is_some_and(|&receiver| exact_java_this_var(il, interner, receiver))
}

/// Exact Java `return this` proof used by self-field body fragments.
pub fn exact_java_return_this(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .java_this_field_place()
        || il.kind(node) != NodeKind::Return
    {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 1 && exact_java_this_var(il, interner, kids[0])
}

/// `(receiver, key, value)` of a first-party exact-safe index assignment.
///
/// This is intentionally language-gated: languages with overloadable/user-dispatched index
/// assignment remain fail-closed until a pack supplies a stronger receiver proof.
pub fn exact_non_overloadable_index_assignment_parts(
    il: &Il,
    node: NodeId,
) -> Option<(NodeId, Option<NodeId>, NodeId)> {
    if !semantics(il.meta.lang)
        .exact_fragments()
        .non_overloadable_index_assignment()
    {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Index {
        return None;
    }
    let target = il.children(kids[0]);
    Some((*target.first()?, target.get(1).copied(), kids[1]))
}

pub fn exact_non_overloadable_index_assignment(il: &Il, node: NodeId) -> bool {
    exact_non_overloadable_index_assignment_parts(il, node).is_some()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModuleSemantics {
    lang: Lang,
}

impl ModuleSemantics {
    /// JavaScript-like lexical scopes can shadow imported module bindings with a
    /// local definition of the same name.
    pub fn js_like_shadowed_module_bindings(self) -> bool {
        matches!(
            self.lang,
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
        )
    }

    /// Sibling-module immutable literal export resolution is modeled for these
    /// first-party module systems.
    pub fn sibling_literal_exports(self) -> bool {
        self.path_spec().is_some()
    }

    /// Java class bodies also contribute static literal bindings keyed by class
    /// names and path-derived class module names.
    pub fn java_class_literal_exports(self) -> bool {
        self.lang == Lang::Java
    }

    /// Java class/type declarations can shadow standard type names such as
    /// `Map`, `List`, `Set`, and `Arrays` in first-party stdlib contracts.
    pub fn java_type_declarations_shadow_stdlib(self) -> bool {
        self.lang == Lang::Java
    }

    /// Go static imports are lowered as namespace facts that can prove package
    /// aliases for selected stdlib-style recognizers.
    pub fn go_import_namespace_facts(self) -> bool {
        self.lang == Lang::Go
    }

    pub fn path_spec(self) -> Option<ModulePathSpec> {
        match self.lang {
            Lang::Python => Some(ModulePathSpec {
                extensions: &["py"],
                separator: ".",
                include_relative_dot: false,
                drop_init_file: true,
                rust_crate_self_aliases: false,
            }),
            Lang::JavaScript | Lang::TypeScript => Some(ModulePathSpec {
                extensions: &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"],
                separator: "/",
                include_relative_dot: true,
                drop_init_file: false,
                rust_crate_self_aliases: false,
            }),
            Lang::Java => Some(ModulePathSpec {
                extensions: &["java"],
                separator: ".",
                include_relative_dot: false,
                drop_init_file: false,
                rust_crate_self_aliases: false,
            }),
            Lang::Rust => Some(ModulePathSpec {
                extensions: &["rs"],
                separator: "::",
                include_relative_dot: false,
                drop_init_file: false,
                rust_crate_self_aliases: true,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModulePathSpec {
    pub extensions: &'static [&'static str],
    pub separator: &'static str,
    pub include_relative_dot: bool,
    pub drop_init_file: bool,
    pub rust_crate_self_aliases: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StdlibSemantics {
    lang: Lang,
}

impl StdlibSemantics {
    pub fn python_collection_factories(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn python_deque_factory(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn java_collection_factories(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn java_map_factories(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn java_primitive_integer_ops(self) -> bool {
        self.lang == Lang::Java
    }

    pub fn ruby_set_factory(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn rust_vec_macro_factory(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_vec_new_factory(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_std_collection_factories(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn rust_std_map_factories(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn go_literal_zero_map_lookup(self) -> bool {
        self.lang == Lang::Go
    }

    pub fn rust_filter_map_option_contract(self) -> bool {
        self.lang == Lang::Rust
    }

    pub fn imported_map_factory(self) -> Option<ImportedMapFactoryContract> {
        match self.lang {
            Lang::Java => Some(ImportedMapFactoryContract::JavaMap),
            Lang::Rust => Some(ImportedMapFactoryContract::RustStdMap),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportedMapFactoryContract {
    JavaMap,
    RustStdMap,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinDemand {
    Eager,
    Reduce,
    AnyAll { all: bool },
    Append,
    ValueOrDefault,
}

pub fn builtin_demand(builtin: Builtin) -> BuiltinDemand {
    match builtin {
        Builtin::Reduce => BuiltinDemand::Reduce,
        Builtin::Any => BuiltinDemand::AnyAll { all: false },
        Builtin::All => BuiltinDemand::AnyAll { all: true },
        Builtin::Append => BuiltinDemand::Append,
        Builtin::ValueOrDefault => BuiltinDemand::ValueOrDefault,
        _ => BuiltinDemand::Eager,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EagerBuiltinContract {
    Len,
    IsEmpty,
    IsNull,
    IsNotNull,
    StartsWith,
    EndsWith,
    Contains,
    Join,
    Abs,
    UnsignedCast32,
    Sum,
    Min,
    Max,
    Range,
    Zip,
    Enumerate,
    Keys,
    Print,
    DictEntry,
    GetOrDefault,
}

pub fn eager_builtin_contract(builtin: Builtin) -> Option<EagerBuiltinContract> {
    Some(match builtin {
        Builtin::Len => EagerBuiltinContract::Len,
        Builtin::IsEmpty => EagerBuiltinContract::IsEmpty,
        Builtin::IsNull => EagerBuiltinContract::IsNull,
        Builtin::IsNotNull => EagerBuiltinContract::IsNotNull,
        Builtin::StartsWith => EagerBuiltinContract::StartsWith,
        Builtin::EndsWith => EagerBuiltinContract::EndsWith,
        Builtin::Contains => EagerBuiltinContract::Contains,
        Builtin::Join => EagerBuiltinContract::Join,
        Builtin::Abs => EagerBuiltinContract::Abs,
        Builtin::UnsignedCast32 => EagerBuiltinContract::UnsignedCast32,
        Builtin::Sum => EagerBuiltinContract::Sum,
        Builtin::Min => EagerBuiltinContract::Min,
        Builtin::Max => EagerBuiltinContract::Max,
        Builtin::Range => EagerBuiltinContract::Range,
        Builtin::Zip => EagerBuiltinContract::Zip,
        Builtin::Enumerate => EagerBuiltinContract::Enumerate,
        Builtin::Keys => EagerBuiltinContract::Keys,
        Builtin::Print => EagerBuiltinContract::Print,
        Builtin::DictEntry => EagerBuiltinContract::DictEntry,
        Builtin::GetOrDefault => EagerBuiltinContract::GetOrDefault,
        Builtin::Reduce
        | Builtin::Any
        | Builtin::All
        | Builtin::Append
        | Builtin::ValueOrDefault => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReductionBuiltinContract {
    Len,
    Sum,
    ExplicitFold,
    Selection { max: bool },
    Bool { all: bool },
    Join,
}

pub fn reduction_builtin_contract(builtin: Builtin) -> Option<ReductionBuiltinContract> {
    Some(match builtin {
        Builtin::Len => ReductionBuiltinContract::Len,
        Builtin::Sum => ReductionBuiltinContract::Sum,
        Builtin::Reduce => ReductionBuiltinContract::ExplicitFold,
        Builtin::Min => ReductionBuiltinContract::Selection { max: false },
        Builtin::Max => ReductionBuiltinContract::Selection { max: true },
        Builtin::Any => ReductionBuiltinContract::Bool { all: false },
        Builtin::All => ReductionBuiltinContract::Bool { all: true },
        Builtin::Join => ReductionBuiltinContract::Join,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HofContract {
    Map,
    FlatMap,
    FilterMap,
    Filter,
    Reduce,
}

pub fn hof_contract(kind: HoFKind) -> HofContract {
    match kind {
        HoFKind::Map => HofContract::Map,
        HoFKind::FlatMap => HofContract::FlatMap,
        HoFKind::FilterMap => HofContract::FilterMap,
        HoFKind::Filter => HofContract::Filter,
        HoFKind::Reduce => HofContract::Reduce,
    }
}

/// The value-graph call tag for a canonical builtin. Tag `0` is reserved for
/// opaque calls, so kernel-owned builtin contracts start at `1`.
pub fn builtin_tag(builtin: Builtin) -> u32 {
    builtin as u32 + 1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinArgContract {
    First,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeFunctionBuiltinContract {
    pub builtin: Builtin,
    pub args: BuiltinArgContract,
    pub requires_unshadowed: bool,
}

pub fn free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<FreeFunctionBuiltinContract> {
    let contract = match name {
        "len" if matches!(lang, Lang::Python | Lang::Go) && arg_count == 1 => {
            (Builtin::Len, BuiltinArgContract::First)
        }
        "append" if lang == Lang::Go && arg_count >= 2 => {
            (Builtin::Append, BuiltinArgContract::All)
        }
        "print" if lang == Lang::Python => (Builtin::Print, BuiltinArgContract::All),
        "range" if lang == Lang::Python => (Builtin::Range, BuiltinArgContract::All),
        "sum" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Sum, BuiltinArgContract::First)
        }
        "min" if lang == Lang::Python && (arg_count == 1 || arg_count == 2) => {
            (Builtin::Min, BuiltinArgContract::All)
        }
        "max" if lang == Lang::Python && (arg_count == 1 || arg_count == 2) => {
            (Builtin::Max, BuiltinArgContract::All)
        }
        "abs" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Abs, BuiltinArgContract::First)
        }
        "zip" if lang == Lang::Python && arg_count == 2 => (Builtin::Zip, BuiltinArgContract::All),
        "enumerate" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Enumerate, BuiltinArgContract::First)
        }
        "any" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::Any, BuiltinArgContract::First)
        }
        "all" if lang == Lang::Python && arg_count == 1 => {
            (Builtin::All, BuiltinArgContract::First)
        }
        _ => return None,
    };
    Some(FreeFunctionBuiltinContract {
        builtin: contract.0,
        args: contract.1,
        requires_unshadowed: true,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodReceiverContract {
    ExactCollection,
    ExactProtocol,
    ExactProtocolPairArgument,
    ExactOption,
    ExactString,
    ExactInteger,
    ExactMap,
    ExactMapLiteral,
    ExactCollectionOrMap,
    ExactCollectionOrMapLiteral,
    ExactCollectionOrJavaKeySet,
    ExactSetOrMap,
    LiteralString,
    UnshadowedGlobal(&'static str),
    ImportedNamespace(&'static str),
    RustMapGetOrExactOption,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodBuiltinArgs {
    All,
    First,
    ReceiverOnly,
    ReceiverThenAll,
    ReceiverAndFirst,
    FirstThenReceiver,
    GoSliceContains,
    MapGetDefault,
    RustMapGetOrOptionDefault,
    RustOptionDefaultLambda,
    RustOptionMapOrIdentity,
    RustZip,
    Fold,
    BoolReduction,
    Hof,
    CollectionReduction,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MethodSemanticContract {
    Builtin(Builtin),
    HoF(HoFKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodCallContract {
    pub semantic: MethodSemanticContract,
    pub receiver: MethodReceiverContract,
    pub args: MethodBuiltinArgs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalarIntegerMethod {
    Abs,
    Min,
    Max,
    Clamp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalarIntegerMethodContract {
    pub semantic: ScalarIntegerMethod,
    pub receiver: MethodReceiverContract,
}

pub fn scalar_integer_method_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<ScalarIntegerMethodContract> {
    use ScalarIntegerMethod as Method;

    let semantic = match (lang, name, arg_count) {
        (Lang::Rust, "abs", 0) => Method::Abs,
        (Lang::Rust, "min", 1) => Method::Min,
        (Lang::Rust, "max", 1) => Method::Max,
        (Lang::Rust, "clamp", 2) => Method::Clamp,
        _ => return None,
    };
    Some(ScalarIntegerMethodContract {
        semantic,
        receiver: MethodReceiverContract::ExactInteger,
    })
}

pub fn method_call_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<MethodCallContract> {
    use MethodBuiltinArgs as Args;
    use MethodReceiverContract as Receiver;
    use MethodSemanticContract as Semantic;

    let contract = match (lang, name, arg_count) {
        (Lang::Python, "append", 1) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "push",
            1..,
        ) => (
            Builtin::Append,
            Receiver::ExactCollection,
            Args::ReceiverThenAll,
        ),

        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "log" | "info" | "debug",
            _,
        ) => (
            Builtin::Print,
            Receiver::UnshadowedGlobal("console"),
            Args::All,
        ),
        (Lang::Go, "Println" | "Printf" | "Print", _) => (
            Builtin::Print,
            Receiver::ImportedNamespace("fmt"),
            Args::All,
        ),
        (Lang::Go, "Abs", 1) => (
            Builtin::Abs,
            Receiver::ImportedNamespace("math"),
            Args::First,
        ),
        (Lang::Go, "HasPrefix", 2) => (
            Builtin::StartsWith,
            Receiver::ImportedNamespace("strings"),
            Args::All,
        ),
        (Lang::Go, "HasSuffix", 2) => (
            Builtin::EndsWith,
            Receiver::ImportedNamespace("strings"),
            Args::All,
        ),
        (Lang::Go, "Contains", 2) => (
            Builtin::Contains,
            Receiver::ImportedNamespace("slices"),
            Args::GoSliceContains,
        ),

        (Lang::Rust, "len", 0) | (Lang::Java, "size", 0) => {
            (Builtin::Len, Receiver::ExactCollection, Args::ReceiverOnly)
        }
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "length",
            0,
        ) => (Builtin::Len, Receiver::ExactCollection, Args::ReceiverOnly),
        (Lang::Rust, "is_empty", 0) | (Lang::Java, "isEmpty", 0) | (Lang::Ruby, "empty?", 0) => (
            Builtin::IsEmpty,
            Receiver::ExactCollection,
            Args::ReceiverOnly,
        ),
        (Lang::Ruby, "nil?", 0) | (Lang::Rust, "is_none", 0) => {
            (Builtin::IsNull, Receiver::ExactOption, Args::ReceiverOnly)
        }
        (Lang::Rust, "is_some", 0) => (
            Builtin::IsNotNull,
            Receiver::RustMapGetOrExactOption,
            Args::ReceiverOnly,
        ),

        (
            Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
            | Lang::Java,
            "startsWith",
            1,
        )
        | (Lang::Python, "startswith", 1)
        | (Lang::Rust, "starts_with", 1)
        | (Lang::Ruby, "start_with?", 1) => (
            Builtin::StartsWith,
            Receiver::ExactString,
            Args::ReceiverAndFirst,
        ),
        (
            Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
            | Lang::Java,
            "endsWith",
            1,
        )
        | (Lang::Python, "endswith", 1)
        | (Lang::Rust, "ends_with", 1)
        | (Lang::Ruby, "end_with?", 1) => (
            Builtin::EndsWith,
            Receiver::ExactString,
            Args::ReceiverAndFirst,
        ),

        (Lang::Java, "containsKey", 1)
        | (Lang::Rust, "contains_key", 1)
        | (Lang::Ruby, "key?" | "has_key?", 1) => (
            Builtin::Contains,
            Receiver::ExactMap,
            Args::FirstThenReceiver,
        ),
        (Lang::Python, "__contains__", 1) => (
            Builtin::Contains,
            Receiver::ExactCollectionOrMap,
            Args::FirstThenReceiver,
        ),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "includes",
            1,
        )
        | (Lang::Ruby, "include?" | "member?", 1)
        | (Lang::Java | Lang::Rust, "contains", 1) => (
            Builtin::Contains,
            Receiver::ExactCollectionOrJavaKeySet,
            Args::FirstThenReceiver,
        ),
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "has", 1) => {
            (
                Builtin::Contains,
                Receiver::ExactSetOrMap,
                Args::FirstThenReceiver,
            )
        }

        (Lang::Python, "join", 1) => (
            Builtin::Join,
            Receiver::LiteralString,
            Args::ReceiverAndFirst,
        ),
        (Lang::Python, "get", 2) | (Lang::Ruby, "fetch", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefault,
        ),
        (Lang::Java, "getOrDefault", 2) => (
            Builtin::GetOrDefault,
            Receiver::ExactMap,
            Args::MapGetDefault,
        ),
        (Lang::Rust, "unwrap_or", 1) => (
            Builtin::ValueOrDefault,
            Receiver::RustMapGetOrExactOption,
            Args::RustMapGetOrOptionDefault,
        ),
        (Lang::Rust, "unwrap_or_else", 1) => (
            Builtin::ValueOrDefault,
            Receiver::ExactOption,
            Args::RustOptionDefaultLambda,
        ),
        (Lang::Rust, "map_or", 2) => (
            Builtin::ValueOrDefault,
            Receiver::ExactOption,
            Args::RustOptionMapOrIdentity,
        ),

        (Lang::Python, "reduce", 2..) => (
            Builtin::Reduce,
            Receiver::ImportedNamespace("functools"),
            Args::All,
        ),
        (Lang::Go, "Min", 2) => (Builtin::Min, Receiver::ImportedNamespace("math"), Args::All),
        (Lang::Go, "Max", 2) => (Builtin::Max, Receiver::ImportedNamespace("math"), Args::All),
        (Lang::Rust, "zip", 1) => (
            Builtin::Zip,
            Receiver::ExactProtocolPairArgument,
            Args::RustZip,
        ),

        _ if method_fold_name(lang, name) && arg_count > 0 => {
            (Builtin::Reduce, Receiver::ExactProtocol, Args::Fold)
        }
        _ if method_bool_reduction_builtin(lang, name).is_some() && arg_count > 0 => (
            method_bool_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactProtocol,
            Args::BoolReduction,
        ),
        _ if method_collection_reduction_builtin(lang, name).is_some() && arg_count == 0 => (
            method_collection_reduction_builtin(lang, name).unwrap(),
            Receiver::ExactProtocol,
            Args::CollectionReduction,
        ),
        _ if method_hof_contract(lang, name).is_some() && arg_count > 0 => {
            return Some(MethodCallContract {
                semantic: Semantic::HoF(method_hof_contract(lang, name).unwrap()),
                receiver: Receiver::ExactProtocol,
                args: Args::Hof,
            });
        }
        (Lang::Rust, "abs", 0) => (Builtin::Abs, Receiver::ExactInteger, Args::ReceiverOnly),
        _ => return None,
    };

    Some(MethodCallContract {
        semantic: Semantic::Builtin(contract.0),
        receiver: contract.1,
        args: contract.2,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsyncReceiverContract {
    ExactPromiseLike,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PromiseThenContract {
    pub receiver: AsyncReceiverContract,
}

pub fn promise_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<PromiseThenContract> {
    if matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    ) && method == "then"
        && arg_count == 1
    {
        Some(PromiseThenContract {
            receiver: AsyncReceiverContract::ExactPromiseLike,
        })
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IteratorAdapterReceiverContract {
    ExactIterableValue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IteratorIdentityAdapterContract {
    pub receiver: IteratorAdapterReceiverContract,
}

pub fn iterator_identity_adapter_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<IteratorIdentityAdapterContract> {
    if lang == Lang::Rust
        && arg_count == 0
        && matches!(
            method,
            "iter" | "into_iter" | "iter_mut" | "collect" | "to_vec" | "copied" | "cloned"
        )
    {
        Some(IteratorIdentityAdapterContract {
            receiver: IteratorAdapterReceiverContract::ExactIterableValue,
        })
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowedPathContract {
    pub shadow_root: &'static str,
}

pub fn rust_option_some_constructor_contract(
    lang: Lang,
    name: &str,
) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Some" => "Some",
        "Option::Some" => "Option",
        "std::option::Option::Some" => "std",
        "core::option::Option::Some" => "core",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_vec_new_factory_contract(lang: Lang, name: &str) -> Option<ShadowedPathContract> {
    if lang != Lang::Rust {
        return None;
    }
    let shadow_root = match name {
        "Vec::new" => "Vec",
        "std::vec::Vec::new" => "std",
        "alloc::vec::Vec::new" => "alloc",
        _ => return None,
    };
    Some(ShadowedPathContract { shadow_root })
}

pub fn rust_option_and_then_contract(lang: Lang, method: &str, arg_count: usize) -> bool {
    lang == Lang::Rust && method == "and_then" && arg_count == 1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaCollectionFactoryKind {
    ListOf,
    SetOf,
    ArraysAsList,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaCollectionFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub kind: JavaCollectionFactoryKind,
    pub single_arg_spreads_array: bool,
}

pub fn java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<JavaCollectionFactoryContract> {
    if lang != Lang::Java {
        return None;
    }
    Some(match (receiver, method) {
        ("List", "of") => JavaCollectionFactoryContract {
            receiver: "List",
            method: "of",
            kind: JavaCollectionFactoryKind::ListOf,
            single_arg_spreads_array: false,
        },
        ("Set", "of") => JavaCollectionFactoryContract {
            receiver: "Set",
            method: "of",
            kind: JavaCollectionFactoryKind::SetOf,
            single_arg_spreads_array: false,
        },
        ("Arrays", "asList") => JavaCollectionFactoryContract {
            receiver: "Arrays",
            method: "asList",
            kind: JavaCollectionFactoryKind::ArraysAsList,
            single_arg_spreads_array: true,
        },
        _ => return None,
    })
}

pub fn java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<JavaCollectionFactoryContract> {
    ["of", "asList"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JavaMapFactoryKind {
    Of,
    OfEntries,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JavaMapFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub kind: JavaMapFactoryKind,
}

pub fn java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<JavaMapFactoryContract> {
    if lang != Lang::Java || receiver != "Map" {
        return None;
    }
    Some(match method {
        "of" => JavaMapFactoryContract {
            receiver: "Map",
            method: "of",
            kind: JavaMapFactoryKind::Of,
        },
        "ofEntries" => JavaMapFactoryContract {
            receiver: "Map",
            method: "ofEntries",
            kind: JavaMapFactoryKind::OfEntries,
        },
        _ => return None,
    })
}

pub fn java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<JavaMapFactoryContract> {
    ["of", "ofEntries"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| java_map_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn java_map_entry_contract(lang: Lang, receiver: &str, method: &str) -> bool {
    lang == Lang::Java && receiver == "Map" && method == "entry"
}

pub fn java_map_entry_contract_by_hash(lang: Lang, receiver: &str, method_hash: u64) -> bool {
    java_map_entry_contract(lang, receiver, "entry") && method_hash == stable_symbol_hash("entry")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RubySetFactoryContract {
    pub receiver: &'static str,
    pub method: &'static str,
    pub required_module: &'static str,
    pub shadow_root: &'static str,
}

pub fn ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<RubySetFactoryContract> {
    (lang == Lang::Ruby && receiver == "Set" && method == "new" && arg_count == 1).then_some(
        RubySetFactoryContract {
            receiver: "Set",
            method: "new",
            required_module: "set",
            shadow_root: "Set",
        },
    )
}

pub fn ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<RubySetFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstructorProofRequirement {
    ConstructSyntax,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClosedConstructorContract {
    pub receiver: &'static str,
    pub required_proof: ConstructorProofRequirement,
}

pub fn js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<ClosedConstructorContract> {
    (js_like_lang(lang) && receiver == "Set").then_some(ClosedConstructorContract {
        receiver: "Set",
        required_proof: ConstructorProofRequirement::ConstructSyntax,
    })
}

pub fn js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<ClosedConstructorContract> {
    (js_like_lang(lang) && receiver == "Map").then_some(ClosedConstructorContract {
        receiver: "Map",
        required_proof: ConstructorProofRequirement::ConstructSyntax,
    })
}

fn js_like_lang(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKeyViewKind {
    Collection,
    Iterator,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapKeyViewContract {
    pub method: &'static str,
    pub kind: MapKeyViewKind,
}

pub fn map_key_view_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<MapKeyViewContract> {
    if arg_count != 0 {
        return None;
    }
    Some(match (lang, method) {
        (Lang::Python | Lang::Ruby, "keys") => MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::Java, "keySet") => MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "keys") => {
            MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            }
        }
        _ => return None,
    })
}

pub fn map_key_view_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapKeyViewContract> {
    ["keys", "keySet"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| map_key_view_contract(lang, method, arg_count))
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapKeyViewWrapperContract {
    pub receiver: &'static str,
    pub method: &'static str,
}

pub fn map_key_view_wrapper_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<MapKeyViewWrapperContract> {
    (js_like_lang(lang) && receiver == "Array" && method == "from" && arg_count == 1).then_some(
        MapKeyViewWrapperContract {
            receiver: "Array",
            method: "from",
        },
    )
}

pub fn map_key_view_wrapper_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapKeyViewWrapperContract> {
    (method_hash == stable_symbol_hash("from"))
        .then(|| map_key_view_wrapper_contract(lang, receiver, "from", arg_count))
        .flatten()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GoZeroMapLookupContract {
    pub map_literal_tag: &'static str,
    pub entry_tag: &'static str,
    pub canonical_value_tag: &'static str,
}

pub fn go_zero_map_lookup_contract(lang: Lang) -> Option<GoZeroMapLookupContract> {
    (lang == Lang::Go).then_some(GoZeroMapLookupContract {
        map_literal_tag: "composite_literal",
        entry_tag: "keyed_element",
        canonical_value_tag: "go_literal_zero_map",
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GoZeroMapDefaultKind {
    Int,
    String,
    Bool,
    Float,
    Null,
}

pub fn go_zero_map_default_kind(lang: Lang, payload: Payload) -> Option<GoZeroMapDefaultKind> {
    if lang != Lang::Go {
        return None;
    }
    Some(match payload {
        Payload::LitInt(_) => GoZeroMapDefaultKind::Int,
        Payload::LitStr(_) => GoZeroMapDefaultKind::String,
        Payload::LitBool(_) => GoZeroMapDefaultKind::Bool,
        Payload::LitFloat(_) => GoZeroMapDefaultKind::Float,
        Payload::Lit(LitClass::Null) => GoZeroMapDefaultKind::Null,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapGetContract {
    pub method: &'static str,
}

pub fn map_get_contract(lang: Lang, method: &str, arg_count: usize) -> Option<MapGetContract> {
    matches!(
        lang,
        Lang::Java
            | Lang::Rust
            | Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
    )
    .then_some(())
    .filter(|_| method == "get" && arg_count == 1)
    .map(|_| MapGetContract { method: "get" })
}

pub fn map_get_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<MapGetContract> {
    (method_hash == stable_symbol_hash("get"))
        .then(|| map_get_contract(lang, "get", arg_count))
        .flatten()
}

pub fn builder_append_method_contract(lang: Lang, method: &str, arg_count: usize) -> bool {
    matches!(
        (lang, method, arg_count),
        (Lang::Python, "append", 1)
            | (
                Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
                "push",
                1
            )
            | (Lang::Java, "add", 1)
            | (Lang::Rust, "push", 1)
    )
}

/// `(receiver, value)` of a single-item append-like builder call admitted by first-party
/// language/library contracts.
pub fn builder_append_call_args(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) {
        return (kids.len() == 2).then(|| (kids[0], kids[1]));
    }
    let (&callee, args) = kids.split_first()?;
    if args.len() != 1 || il.kind(callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    if !builder_append_method_contract(il.meta.lang, interner.resolve(method), args.len()) {
        return None;
    }
    let receiver = *il.children(callee).first()?;
    Some((receiver, args[0]))
}

pub fn builder_append_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    builder_append_call_args(il, interner, node).is_some()
}

pub fn method_fold_name(lang: Lang, name: &str) -> bool {
    matches!(
        (lang, name),
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "reduce"
        ) | (Lang::Ruby, "inject" | "reduce")
            | (Lang::Rust, "fold")
            | (Lang::Java, "reduce")
    )
}

pub fn method_bool_reduction_builtin(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "some") => {
            Builtin::Any
        }
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "every") => {
            Builtin::All
        }
        (Lang::Rust, "any") | (Lang::Ruby, "any?") | (Lang::Java, "anyMatch") => Builtin::Any,
        (Lang::Rust, "all") | (Lang::Ruby, "all?") | (Lang::Java, "allMatch") => Builtin::All,
        _ => return None,
    })
}

pub fn method_hof_contract(lang: Lang, name: &str) -> Option<HoFKind> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "map")
        | (Lang::Rust, "map")
        | (Lang::Java, "map")
        | (Lang::Ruby, "map" | "collect") => HoFKind::Map,
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            "flatMap",
        )
        | (Lang::Rust, "flat_map")
        | (Lang::Java, "flatMap") => HoFKind::FlatMap,
        (Lang::Rust, "filter_map") => HoFKind::FilterMap,
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "filter")
        | (Lang::Rust, "filter")
        | (Lang::Java, "filter")
        | (Lang::Ruby, "filter" | "select") => HoFKind::Filter,
        _ => return None,
    })
}

pub fn method_collection_reduction_builtin(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::Rust, "sum") => Builtin::Sum,
        (Lang::Rust, "min") => Builtin::Min,
        (Lang::Rust, "max") => Builtin::Max,
        (Lang::Rust, "count") => Builtin::Len,
        (Lang::Java, "count") => Builtin::Len,
        _ => return None,
    })
}

pub fn property_builtin_contract(lang: Lang, name: &str) -> Option<Builtin> {
    Some(match (lang, name) {
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "length") => {
            Builtin::Len
        }
        (Lang::Java, "length") => Builtin::Len,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CollectionSemantics {
    lang: Lang,
}

impl CollectionSemantics {
    /// Python's empty `Seq(0)` literal is a collection value for first-party exact
    /// collection contracts.
    pub fn empty_sequence_is_collection(self) -> bool {
        self.lang == Lang::Python
    }

    pub fn ruby_shovel_list_append(self) -> bool {
        self.lang == Lang::Ruby
    }

    pub fn free_name_collection_factories(self) -> impl Iterator<Item = FreeNameCollectionFactory> {
        FREE_NAME_COLLECTION_FACTORIES
            .iter()
            .copied()
            .filter(move |row| row.lang.is_none_or(|lang| lang == self.lang))
    }

    pub fn free_name_map_factories(self) -> impl Iterator<Item = FreeNameMapFactory> {
        FREE_NAME_MAP_FACTORIES
            .iter()
            .copied()
            .filter(move |row| row.lang.is_none_or(|lang| lang == self.lang))
    }

    pub fn imported_collection_factories(self) -> impl Iterator<Item = ImportedCollectionFactory> {
        IMPORTED_COLLECTION_FACTORIES
            .iter()
            .copied()
            .filter(move |row| row.lang.is_none_or(|lang| lang == self.lang))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeNameCollectionFactory {
    pub lang: Option<Lang>,
    pub names: &'static [&'static str],
    pub shadow_guard: bool,
}

const FREE_NAME_COLLECTION_FACTORIES: &[FreeNameCollectionFactory] = &[
    FreeNameCollectionFactory {
        lang: Some(Lang::Python),
        names: &["list", "set", "frozenset", "tuple"],
        shadow_guard: true,
    },
    FreeNameCollectionFactory {
        lang: Some(Lang::Rust),
        names: &[
            "std::collections::HashSet::from",
            "std::collections::BTreeSet::from",
            "std::collections::VecDeque::from",
        ],
        shadow_guard: false,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeNameMapFactory {
    pub lang: Option<Lang>,
    pub names: &'static [&'static str],
    pub entry_seq_tag: u64,
}

const FREE_NAME_MAP_FACTORIES: &[FreeNameMapFactory] = &[FreeNameMapFactory {
    lang: Some(Lang::Rust),
    names: &[
        "std::collections::HashMap::from",
        "std::collections::BTreeMap::from",
    ],
    entry_seq_tag: 2,
}];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportedCollectionFactory {
    pub lang: Option<Lang>,
    pub module: &'static str,
    pub exported: &'static str,
}

const IMPORTED_COLLECTION_FACTORIES: &[ImportedCollectionFactory] = &[ImportedCollectionFactory {
    lang: Some(Lang::Python),
    module: "collections",
    exported: "deque",
}];

pub fn imported_literal_seq_tag_safe(tag: &str) -> bool {
    matches!(
        tag,
        "dictionary" | "object" | "array" | "array_expression" | "tuple_expression"
    )
}

pub fn mutating_method_name(method: &str) -> bool {
    matches!(
        method,
        "clear"
            | "delete"
            | "insert"
            | "pop"
            | "popitem"
            | "put"
            | "putAll"
            | "remove"
            | "set"
            | "setdefault"
            | "update"
    )
}

pub fn module_binding_mutating_method_name(method: &str) -> bool {
    matches!(
        method,
        "add"
            | "addAll"
            | "append"
            | "delete"
            | "clear"
            | "compute"
            | "computeIfAbsent"
            | "computeIfPresent"
            | "merge"
            | "pop"
            | "push"
            | "put"
            | "putAll"
            | "remove"
            | "removeAll"
            | "removeIf"
            | "replace"
            | "replaceAll"
            | "retainAll"
            | "shift"
            | "sort"
            | "splice"
            | "unshift"
            | "set"
    )
}

pub fn async_to_sync_name(lang: Lang, name: &str) -> Option<&'static str> {
    if lang != Lang::Python {
        return None;
    }
    Some(match name {
        "__aenter__" => "__enter__",
        "__aexit__" => "__exit__",
        "__anext__" => "__next__",
        "__aiter__" => "__iter__",
        "aread" => "read",
        "areadline" => "readline",
        "areadlines" => "readlines",
        "awrite" => "write",
        "aclose" => "close",
        "asend" => "send",
        "areceive" => "receive",
        "aconnect" => "connect",
        "adrain" => "drain",
        "aflush" => "flush",
        "AsyncIterable" => "Iterable",
        "AsyncIterator" => "Iterator",
        "AsyncGenerator" => "Generator",
        "AsyncContextManager" => "ContextManager",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, ParamSemantic, Span, Unit, UnitKind};

    const ALL_LANGS: &[Lang] = &[
        Lang::Python,
        Lang::JavaScript,
        Lang::TypeScript,
        Lang::Go,
        Lang::Rust,
        Lang::Java,
        Lang::C,
        Lang::Ruby,
        Lang::Vue,
        Lang::Svelte,
        Lang::Html,
    ];

    const ALL_BUILTINS: &[Builtin] = &[
        Builtin::Len,
        Builtin::Print,
        Builtin::Append,
        Builtin::Range,
        Builtin::Sum,
        Builtin::Reduce,
        Builtin::Min,
        Builtin::Max,
        Builtin::Abs,
        Builtin::Zip,
        Builtin::Enumerate,
        Builtin::Keys,
        Builtin::Any,
        Builtin::All,
        Builtin::DictEntry,
        Builtin::IsEmpty,
        Builtin::StartsWith,
        Builtin::EndsWith,
        Builtin::Contains,
        Builtin::GetOrDefault,
        Builtin::ValueOrDefault,
        Builtin::IsNull,
        Builtin::IsNotNull,
        Builtin::Join,
        Builtin::UnsignedCast32,
    ];

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, 1, 1)
    }

    fn finish_il(builder: IlBuilder, root: NodeId, lang: Lang) -> Il {
        builder.finish(
            root,
            FileMeta {
                path: "t".into(),
                lang,
            },
            vec![Unit {
                root,
                kind: UnitKind::Function,
                name: None,
            }],
            Vec::new(),
        )
    }

    #[test]
    fn first_party_profile_wraps_each_language() {
        for &lang in ALL_LANGS {
            let profile = semantics(lang);
            assert_eq!(profile.lang(), lang);
            assert_eq!(profile.pack_id(), FIRST_PARTY_PACK_ID);
            assert_eq!(profile.eligibility(), ChannelEligibility::FirstParty);
        }
    }

    #[test]
    fn domain_evidence_preserves_param_semantic_boundaries() {
        assert_eq!(
            domain_evidence_from_param_semantic(ParamSemantic::Array),
            DomainEvidence::Array
        );
        assert!(DomainEvidence::Array.is_array_collection_or_set());
        assert!(DomainEvidence::Collection.is_array_or_collection());
        assert!(DomainEvidence::Set.is_collection_or_set());
        assert!(DomainEvidence::Map.is_map());
        assert!(DomainEvidence::Option.is_option());
        assert!(DomainEvidence::String.is_string());
        assert!(DomainEvidence::ByteArray.is_byte_array());
        assert!(DomainEvidence::Integer.is_integer());
        assert!(DomainEvidence::Number.is_integer_or_number());
        assert!(DomainEvidence::Integer.is_integer_or_number());
        assert!(!DomainEvidence::Number.is_integer());
        assert!(!DomainEvidence::Array.is_collection_or_set());
        assert!(!DomainEvidence::Set.is_array_or_collection());
    }

    #[test]
    fn language_predicates_preserve_existing_gates() {
        for &lang in ALL_LANGS {
            let profile = semantics(lang);
            assert_eq!(
                profile.operators().primitive_order_comparisons(),
                matches!(lang, Lang::C | Lang::Go | Lang::Java)
            );
            assert_eq!(
                profile.operators().c_integer_byte_pack_contracts(),
                lang == Lang::C
            );
            assert_eq!(
                profile.effects().non_overloadable_index_assignment(),
                matches!(lang, Lang::C | Lang::Go | Lang::Java)
            );
            assert_eq!(
                profile.effects().java_this_field_place(),
                lang == Lang::Java
            );
            assert_eq!(
                profile.modules().js_like_shadowed_module_bindings(),
                matches!(
                    lang,
                    Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html
                )
            );
            assert_eq!(
                profile.modules().java_class_literal_exports(),
                lang == Lang::Java
            );
            assert_eq!(
                profile.modules().java_type_declarations_shadow_stdlib(),
                lang == Lang::Java
            );
            assert_eq!(
                profile.modules().go_import_namespace_facts(),
                lang == Lang::Go
            );
        }
    }

    #[test]
    fn stdlib_predicates_preserve_existing_gates() {
        for &lang in ALL_LANGS {
            let stdlib = semantics(lang).stdlib();
            assert_eq!(stdlib.python_collection_factories(), lang == Lang::Python);
            assert_eq!(stdlib.python_deque_factory(), lang == Lang::Python);
            assert_eq!(stdlib.java_collection_factories(), lang == Lang::Java);
            assert_eq!(stdlib.java_map_factories(), lang == Lang::Java);
            assert_eq!(stdlib.java_primitive_integer_ops(), lang == Lang::Java);
            assert_eq!(stdlib.ruby_set_factory(), lang == Lang::Ruby);
            assert_eq!(stdlib.rust_vec_macro_factory(), lang == Lang::Rust);
            assert_eq!(stdlib.rust_vec_new_factory(), lang == Lang::Rust);
            assert_eq!(stdlib.rust_std_collection_factories(), lang == Lang::Rust);
            assert_eq!(stdlib.rust_std_map_factories(), lang == Lang::Rust);
            assert_eq!(stdlib.go_literal_zero_map_lookup(), lang == Lang::Go);
            assert_eq!(stdlib.rust_filter_map_option_contract(), lang == Lang::Rust);
        }
    }

    #[test]
    fn free_name_contracts_are_behavior_equivalent_tables() {
        let py_names: Vec<_> = semantics(Lang::Python)
            .collections()
            .free_name_collection_factories()
            .flat_map(|factory| factory.names.iter().copied())
            .collect();
        assert!(py_names.contains(&"list"));
        assert!(py_names.contains(&"frozenset"));
        assert!(!py_names.contains(&"Set"));

        let imported_py_names: Vec<_> = semantics(Lang::Python)
            .collections()
            .imported_collection_factories()
            .map(|factory| (factory.module, factory.exported))
            .collect();
        assert_eq!(imported_py_names, vec![("collections", "deque")]);

        let rust_map_tags: Vec<_> = semantics(Lang::Rust)
            .collections()
            .free_name_map_factories()
            .map(|factory| factory.entry_seq_tag)
            .collect();
        assert_eq!(rust_map_tags, vec![2]);

        let js_map_tags: Vec<_> = semantics(Lang::JavaScript)
            .collections()
            .free_name_map_factories()
            .map(|factory| factory.entry_seq_tag)
            .collect();
        assert!(js_map_tags.is_empty());
    }

    #[test]
    fn mutating_method_sets_stay_distinct() {
        assert!(mutating_method_name("put"));
        assert!(!mutating_method_name("push"));
        assert!(module_binding_mutating_method_name("push"));
        assert!(module_binding_mutating_method_name("addAll"));
    }

    #[test]
    fn builtin_contracts_preserve_current_special_demand_split() {
        for &builtin in ALL_BUILTINS {
            assert_eq!(builtin_tag(builtin), builtin as u32 + 1);
        }
        assert_eq!(builtin_demand(Builtin::Reduce), BuiltinDemand::Reduce);
        assert_eq!(
            builtin_demand(Builtin::Any),
            BuiltinDemand::AnyAll { all: false }
        );
        assert_eq!(
            builtin_demand(Builtin::All),
            BuiltinDemand::AnyAll { all: true }
        );
        assert_eq!(builtin_demand(Builtin::Append), BuiltinDemand::Append);
        assert_eq!(
            builtin_demand(Builtin::ValueOrDefault),
            BuiltinDemand::ValueOrDefault
        );
        assert_eq!(builtin_demand(Builtin::Len), BuiltinDemand::Eager);
        assert_eq!(
            eager_builtin_contract(Builtin::Len),
            Some(EagerBuiltinContract::Len)
        );
        assert_eq!(eager_builtin_contract(Builtin::Append), None);
        assert_eq!(
            reduction_builtin_contract(Builtin::Max),
            Some(ReductionBuiltinContract::Selection { max: true })
        );
        assert_eq!(
            reduction_builtin_contract(Builtin::Any),
            Some(ReductionBuiltinContract::Bool { all: false })
        );
        assert_eq!(reduction_builtin_contract(Builtin::Print), None);
        assert_eq!(hof_contract(HoFKind::FilterMap), HofContract::FilterMap);
    }

    #[test]
    fn free_function_builtin_contracts_are_language_and_shadow_constrained() {
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "len", 1),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Len,
                args: BuiltinArgContract::First,
                requires_unshadowed: true,
            })
        );
        assert_eq!(free_function_builtin_contract(Lang::Python, "len", 2), None);
        assert_eq!(
            free_function_builtin_contract(Lang::JavaScript, "len", 1),
            None
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "print", 3),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Print,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Go, "append", 2),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Append,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(free_function_builtin_contract(Lang::Go, "append", 1), None);
        assert_eq!(free_function_builtin_contract(Lang::C, "fmaxf", 2), None);
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "fmaxf", 2),
            None
        );
        assert_eq!(
            free_function_builtin_contract(Lang::Python, "max", 2),
            Some(FreeFunctionBuiltinContract {
                builtin: Builtin::Max,
                args: BuiltinArgContract::All,
                requires_unshadowed: true,
            })
        );
        assert_eq!(free_function_builtin_contract(Lang::Python, "any", 2), None);
    }

    #[test]
    fn method_protocol_contracts_are_language_constrained() {
        assert!(method_fold_name(Lang::Ruby, "inject"));
        assert!(!method_fold_name(Lang::Python, "inject"));
        assert!(!method_fold_name(Lang::Ruby, "map"));
        assert_eq!(
            method_bool_reduction_builtin(Lang::Java, "anyMatch"),
            Some(Builtin::Any)
        );
        assert_eq!(
            method_bool_reduction_builtin(Lang::JavaScript, "every"),
            Some(Builtin::All)
        );
        assert_eq!(method_bool_reduction_builtin(Lang::Python, "every"), None);
        assert_eq!(
            method_hof_contract(Lang::Ruby, "collect"),
            Some(HoFKind::Map)
        );
        assert_eq!(
            method_hof_contract(Lang::Rust, "flat_map"),
            Some(HoFKind::FlatMap)
        );
        assert_eq!(
            method_hof_contract(Lang::Ruby, "select"),
            Some(HoFKind::Filter)
        );
        assert_eq!(method_hof_contract(Lang::Python, "select"), None);
        assert_eq!(
            method_collection_reduction_builtin(Lang::Rust, "count"),
            Some(Builtin::Len)
        );
        assert_eq!(
            method_collection_reduction_builtin(Lang::Java, "count"),
            Some(Builtin::Len)
        );
        assert_eq!(
            method_collection_reduction_builtin(Lang::JavaScript, "count"),
            None
        );
        assert_eq!(
            property_builtin_contract(Lang::JavaScript, "length"),
            Some(Builtin::Len)
        );
        assert_eq!(property_builtin_contract(Lang::Python, "length"), None);
    }

    #[test]
    fn method_call_contracts_carry_receiver_and_resolution_obligations() {
        assert_eq!(
            method_call_contract(Lang::Python, "append", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Append),
                receiver: MethodReceiverContract::ExactCollection,
                args: MethodBuiltinArgs::ReceiverThenAll,
            })
        );
        assert_eq!(method_call_contract(Lang::Python, "append", 0), None);
        assert_eq!(
            method_call_contract(Lang::JavaScript, "log", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Print),
                receiver: MethodReceiverContract::UnshadowedGlobal("console"),
                args: MethodBuiltinArgs::All,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Go, "Abs", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Abs),
                receiver: MethodReceiverContract::ImportedNamespace("math"),
                args: MethodBuiltinArgs::First,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Python, "__contains__", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactCollectionOrMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            })
        );
        assert_eq!(
            method_call_contract(Lang::TypeScript, "has", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactSetOrMap,
                args: MethodBuiltinArgs::FirstThenReceiver,
            })
        );
        assert_eq!(
            method_call_contract(Lang::Ruby, "member?", 1),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::Contains),
                receiver: MethodReceiverContract::ExactCollectionOrJavaKeySet,
                args: MethodBuiltinArgs::FirstThenReceiver,
            })
        );
        assert_eq!(method_call_contract(Lang::JavaScript, "contains", 1), None);
        assert_eq!(
            method_call_contract(Lang::Java, "getOrDefault", 2),
            Some(MethodCallContract {
                semantic: MethodSemanticContract::Builtin(Builtin::GetOrDefault),
                receiver: MethodReceiverContract::ExactMap,
                args: MethodBuiltinArgs::MapGetDefault,
            })
        );
        assert_eq!(method_call_contract(Lang::JavaScript, "abs", 0), None);
    }

    #[test]
    fn scalar_integer_methods_are_language_and_signature_constrained() {
        assert_eq!(
            scalar_integer_method_contract(Lang::Rust, "clamp", 2),
            Some(ScalarIntegerMethodContract {
                semantic: ScalarIntegerMethod::Clamp,
                receiver: MethodReceiverContract::ExactInteger,
            })
        );
        assert_eq!(
            scalar_integer_method_contract(Lang::Rust, "min", 1),
            Some(ScalarIntegerMethodContract {
                semantic: ScalarIntegerMethod::Min,
                receiver: MethodReceiverContract::ExactInteger,
            })
        );
        assert_eq!(scalar_integer_method_contract(Lang::Rust, "clamp", 1), None);
        assert_eq!(
            scalar_integer_method_contract(Lang::TypeScript, "clamp", 2),
            None
        );
        assert_eq!(
            scalar_integer_method_contract(Lang::JavaScript, "abs", 0),
            None
        );
    }

    #[test]
    fn async_to_sync_contracts_are_python_constrained() {
        assert_eq!(
            async_to_sync_name(Lang::Python, "__aenter__"),
            Some("__enter__")
        );
        assert_eq!(async_to_sync_name(Lang::Python, "aread"), Some("read"));
        assert_eq!(
            async_to_sync_name(Lang::Python, "AsyncIterator"),
            Some("Iterator")
        );
        assert_eq!(async_to_sync_name(Lang::JavaScript, "aread"), None);
        assert_eq!(async_to_sync_name(Lang::Python, "append"), None);
    }

    #[test]
    fn promise_then_contract_requires_js_like_surface_and_receiver_proof() {
        assert_eq!(
            promise_then_contract(Lang::TypeScript, "then", 1),
            Some(PromiseThenContract {
                receiver: AsyncReceiverContract::ExactPromiseLike,
            })
        );
        assert_eq!(promise_then_contract(Lang::TypeScript, "then", 2), None);
        assert_eq!(promise_then_contract(Lang::Python, "then", 1), None);
    }

    #[test]
    fn iterator_identity_adapters_are_rust_and_receiver_proof_constrained() {
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Rust, "iter", 0),
            Some(IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            })
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Rust, "collect", 0),
            Some(IteratorIdentityAdapterContract {
                receiver: IteratorAdapterReceiverContract::ExactIterableValue,
            })
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::JavaScript, "collect", 0),
            None
        );
        assert_eq!(
            iterator_identity_adapter_contract(Lang::Rust, "collect", 1),
            None
        );
    }

    #[test]
    fn rust_std_path_contracts_carry_shadow_roots() {
        assert_eq!(
            rust_option_some_constructor_contract(Lang::Rust, "Option::Some"),
            Some(ShadowedPathContract {
                shadow_root: "Option",
            })
        );
        assert_eq!(
            rust_option_some_constructor_contract(Lang::Rust, "std::option::Option::Some"),
            Some(ShadowedPathContract { shadow_root: "std" })
        );
        assert_eq!(
            rust_option_some_constructor_contract(Lang::Python, "Some"),
            None
        );
        assert_eq!(
            rust_vec_new_factory_contract(Lang::Rust, "alloc::vec::Vec::new"),
            Some(ShadowedPathContract {
                shadow_root: "alloc",
            })
        );
        assert_eq!(
            rust_vec_new_factory_contract(Lang::Rust, "Vec::with_capacity"),
            None
        );
        assert!(rust_option_and_then_contract(Lang::Rust, "and_then", 1));
        assert!(!rust_option_and_then_contract(Lang::Rust, "and_then", 0));
        assert!(!rust_option_and_then_contract(
            Lang::JavaScript,
            "and_then",
            1
        ));
    }

    #[test]
    fn java_factory_contracts_are_language_receiver_and_selector_constrained() {
        assert_eq!(
            java_collection_factory_contract(Lang::Java, "List", "of"),
            Some(JavaCollectionFactoryContract {
                receiver: "List",
                method: "of",
                kind: JavaCollectionFactoryKind::ListOf,
                single_arg_spreads_array: false,
            })
        );
        assert_eq!(
            java_collection_factory_contract(Lang::Java, "Arrays", "asList"),
            Some(JavaCollectionFactoryContract {
                receiver: "Arrays",
                method: "asList",
                kind: JavaCollectionFactoryKind::ArraysAsList,
                single_arg_spreads_array: true,
            })
        );
        assert_eq!(
            java_collection_factory_contract(Lang::JavaScript, "List", "of"),
            None
        );
        assert_eq!(
            java_collection_factory_contract(Lang::Java, "Map", "of"),
            None
        );
        assert_eq!(
            java_map_factory_contract(Lang::Java, "Map", "ofEntries"),
            Some(JavaMapFactoryContract {
                receiver: "Map",
                method: "ofEntries",
                kind: JavaMapFactoryKind::OfEntries,
            })
        );
        assert_eq!(java_map_factory_contract(Lang::Java, "List", "of"), None);
        assert!(java_map_entry_contract(Lang::Java, "Map", "entry"));
        assert!(!java_map_entry_contract(Lang::Java, "Entry", "entry"));
        assert_eq!(
            java_collection_factory_contract_by_hash(Lang::Java, "Set", stable_symbol_hash("of"))
                .map(|contract| contract.kind),
            Some(JavaCollectionFactoryKind::SetOf)
        );
        assert_eq!(
            java_map_factory_contract_by_hash(Lang::Java, "Map", stable_symbol_hash("of"))
                .map(|contract| contract.kind),
            Some(JavaMapFactoryKind::Of)
        );
        assert!(java_map_entry_contract_by_hash(
            Lang::Java,
            "Map",
            stable_symbol_hash("entry")
        ));
    }

    #[test]
    fn ruby_and_closed_js_like_factory_contracts_keep_proof_obligations_explicit() {
        assert_eq!(
            ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1),
            Some(RubySetFactoryContract {
                receiver: "Set",
                method: "new",
                required_module: "set",
                shadow_root: "Set",
            })
        );
        assert_eq!(ruby_set_factory_contract(Lang::Ruby, "Set", "new", 2), None);
        assert_eq!(
            ruby_set_factory_contract(Lang::Python, "Set", "new", 1),
            None
        );
        assert!(
            ruby_set_factory_contract_by_hash(Lang::Ruby, "Set", stable_symbol_hash("new"), 1)
                .is_some()
        );

        assert_eq!(
            js_like_set_constructor_contract(Lang::TypeScript, "Set"),
            Some(ClosedConstructorContract {
                receiver: "Set",
                required_proof: ConstructorProofRequirement::ConstructSyntax,
            })
        );
        assert_eq!(
            js_like_map_constructor_contract(Lang::JavaScript, "Map"),
            Some(ClosedConstructorContract {
                receiver: "Map",
                required_proof: ConstructorProofRequirement::ConstructSyntax,
            })
        );
        assert_eq!(js_like_map_constructor_contract(Lang::Java, "Map"), None);
        assert_eq!(
            js_like_set_constructor_contract(Lang::JavaScript, "WeakSet"),
            None
        );
    }

    #[test]
    fn map_key_view_contracts_distinguish_collection_and_iterator_views() {
        assert_eq!(
            map_key_view_contract(Lang::Python, "keys", 0),
            Some(MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Collection,
            })
        );
        assert_eq!(
            map_key_view_contract(Lang::Java, "keySet", 0),
            Some(MapKeyViewContract {
                method: "keySet",
                kind: MapKeyViewKind::Collection,
            })
        );
        assert_eq!(
            map_key_view_contract(Lang::TypeScript, "keys", 0),
            Some(MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            })
        );
        assert_eq!(map_key_view_contract(Lang::JavaScript, "keySet", 0), None);
        assert_eq!(map_key_view_contract(Lang::Python, "keys", 1), None);
        assert_eq!(
            map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
            Some(MapKeyViewWrapperContract {
                receiver: "Array",
                method: "from",
            })
        );
        assert_eq!(
            map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
            None
        );
        assert_eq!(
            map_key_view_contract_by_hash(Lang::Java, stable_symbol_hash("keySet"), 0)
                .map(|contract| contract.kind),
            Some(MapKeyViewKind::Collection)
        );
        assert!(map_key_view_wrapper_contract_by_hash(
            Lang::TypeScript,
            "Array",
            stable_symbol_hash("from"),
            1,
        )
        .is_some());
    }

    #[test]
    fn go_zero_map_contracts_are_go_surface_and_default_constrained() {
        assert_eq!(
            go_zero_map_lookup_contract(Lang::Go),
            Some(GoZeroMapLookupContract {
                map_literal_tag: "composite_literal",
                entry_tag: "keyed_element",
                canonical_value_tag: "go_literal_zero_map",
            })
        );
        assert_eq!(go_zero_map_lookup_contract(Lang::Python), None);
        assert_eq!(
            go_zero_map_default_kind(Lang::Go, Payload::LitInt(1)),
            Some(GoZeroMapDefaultKind::Int)
        );
        assert_eq!(
            go_zero_map_default_kind(Lang::Go, Payload::LitStr(stable_symbol_hash("x"))),
            Some(GoZeroMapDefaultKind::String)
        );
        assert_eq!(
            go_zero_map_default_kind(Lang::Go, Payload::Lit(LitClass::Null)),
            Some(GoZeroMapDefaultKind::Null)
        );
        assert_eq!(
            go_zero_map_default_kind(Lang::JavaScript, Payload::LitInt(1)),
            None
        );
        assert_eq!(go_zero_map_default_kind(Lang::Go, Payload::None), None);
    }

    #[test]
    fn map_get_contracts_are_language_and_arity_constrained() {
        assert_eq!(
            map_get_contract(Lang::Rust, "get", 1),
            Some(MapGetContract { method: "get" })
        );
        assert_eq!(
            map_get_contract_by_hash(Lang::Java, stable_symbol_hash("get"), 1),
            Some(MapGetContract { method: "get" })
        );
        assert_eq!(
            map_get_contract(Lang::TypeScript, "get", 1),
            Some(MapGetContract { method: "get" })
        );
        assert_eq!(map_get_contract(Lang::Python, "get", 1), None);
        assert_eq!(map_get_contract(Lang::Rust, "get", 2), None);
        assert_eq!(map_get_contract(Lang::Java, "getOrDefault", 1), None);
    }

    #[test]
    fn builder_append_contracts_are_language_and_arity_constrained() {
        assert!(builder_append_method_contract(Lang::Rust, "push", 1));
        assert!(!builder_append_method_contract(Lang::Rust, "push", 2));
        assert!(builder_append_method_contract(Lang::Java, "add", 1));
        assert!(builder_append_method_contract(Lang::JavaScript, "push", 1));
        assert!(builder_append_method_contract(Lang::Python, "append", 1));
        assert!(!builder_append_method_contract(Lang::Ruby, "push", 1));
    }

    #[test]
    fn exact_java_this_helpers_are_language_and_shape_constrained() {
        let interner = Interner::default();
        let this = interner.intern("this");
        let field_name = interner.intern("value");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Name(this), sp(1), &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(field_name),
            sp(1),
            &[receiver],
        );
        let ret = b.add(NodeKind::Return, Payload::None, sp(2), &[receiver]);
        let root = b.add(NodeKind::Block, Payload::None, sp(1), &[field, ret]);
        let il = finish_il(b, root, Lang::Java);

        assert!(exact_java_this_var(&il, &interner, receiver));
        assert!(exact_java_this_field(&il, &interner, field));
        assert!(exact_java_return_this(&il, &interner, ret));

        let mut js_il = il.clone();
        js_il.meta.lang = Lang::JavaScript;
        assert!(!exact_java_this_var(&js_il, &interner, receiver));
        assert!(!exact_java_this_field(&js_il, &interner, field));
        assert!(!exact_java_return_this(&js_il, &interner, ret));
    }

    #[test]
    fn exact_index_assignment_parts_are_language_constrained() {
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
        let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(1), &[target, value]);
        let il = finish_il(b, assign, Lang::Go);

        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&il, assign),
            Some((receiver, Some(key), value))
        );
        assert!(exact_non_overloadable_index_assignment(&il, assign));

        let mut ruby_il = il.clone();
        ruby_il.meta.lang = Lang::Ruby;
        assert_eq!(
            exact_non_overloadable_index_assignment_parts(&ruby_il, assign),
            None
        );
        assert!(!exact_non_overloadable_index_assignment(&ruby_il, assign));
    }

    #[test]
    fn builder_append_call_args_are_language_arity_and_receiver_constrained() {
        let interner = Interner::default();
        let append = interner.intern("append");
        let push = interner.intern("push");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
        let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
        let builtin = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp(1),
            &[receiver, value],
        );
        let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
        let call = b.add(NodeKind::Call, Payload::None, sp(2), &[method, value]);
        let push_method = b.add(NodeKind::Field, Payload::Name(push), sp(3), &[receiver]);
        let push_call = b.add(NodeKind::Call, Payload::None, sp(3), &[push_method, value]);
        let root = b.add(
            NodeKind::Block,
            Payload::None,
            sp(1),
            &[builtin, call, push_call],
        );
        let il = finish_il(b, root, Lang::Python);

        assert_eq!(
            builder_append_call_args(&il, &interner, builtin),
            Some((receiver, value))
        );
        assert_eq!(
            builder_append_call_args(&il, &interner, call),
            Some((receiver, value))
        );
        assert_eq!(builder_append_call_args(&il, &interner, push_call), None);

        let mut rust_il = il.clone();
        rust_il.meta.lang = Lang::Rust;
        assert_eq!(
            builder_append_call_args(&rust_il, &interner, push_call),
            Some((receiver, value))
        );
    }
}
