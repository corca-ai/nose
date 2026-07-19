use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct UnitEvidenceFlags(u128);

impl UnitEvidenceFlags {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn of(flag: UnitEvidenceFlag) -> Self {
        Self(flag.bit())
    }

    pub const fn with(self, flag: UnitEvidenceFlag) -> Self {
        Self(self.0 | flag.bit())
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, flag: UnitEvidenceFlag) -> bool {
        self.0 & flag.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = UnitEvidenceFlag> {
        UnitEvidenceFlag::ALL
            .iter()
            .copied()
            .filter(move |flag| self.contains(*flag))
    }
}

impl Serialize for UnitEvidenceFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for UnitEvidenceFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let flags = Vec::<UnitEvidenceFlag>::deserialize(deserializer)?;
        Ok(flags
            .into_iter()
            .fold(UnitEvidenceFlags::empty(), UnitEvidenceFlags::with))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitEvidenceFlag {
    Unknown,
    SameSymbol,
    SameOwner,
    SameReceiverType,
    SameSelfType,
    SameTraitTarget,
    SameExtensionTarget,
    SameRootTag,
    SameSelector,
    DifferentSelectors,
    SelectorExcludedFromProof,
    HasRuntimeBody,
    HasReusableBody,
    DeclarationOnly,
    TypeOnly,
    RuntimeValue,
    Ambient,
    DeclarationFile,
    StubFile,
    EllipsisBody,
    PassOnly,
    AbstractOnly,
    DecoratedBinding,
    HasDefaultBody,
    HasAssociatedType,
    FieldOnly,
    DataShapeOnly,
    SameFieldSet,
    SchemaLike,
    RuntimeValidation,
    ProtocolRequirement,
    ProtocolExtension,
    ConcreteTypeExtension,
    ConstrainedExtension,
    InterfaceDefaultMethod,
    InterfaceStaticMethod,
    InterfacePrivateMethod,
    RecordHeader,
    CompactConstructor,
    EnumConstantBody,
    AliasDeclaration,
    DefinedType,
    FieldTags,
    ActorIsolated,
    SwiftuiView,
    ResultBuilderBody,
    ReturnsView,
    Async,
    Throws,
    TestContext,
    TestFixture,
    TestSuite,
    TestCase,
    TestHook,
    AssertionDsl,
    TableDrivenTest,
    FrameworkHook,
    RailsRouteDsl,
    ActiveRecordValidation,
    ActiveRecordScope,
    MigrationDsl,
    FactoryBotDsl,
    RspecExample,
    MinitestTest,
    ComputedStyleEquivalent,
    SameDeclarationBlock,
    SameAtRuleContext,
    AtRuleContext,
    SingleDeclaration,
    UtilityLikeSelector,
    ResetLikeSelector,
    CustomPropertyToken,
    EmbeddedStyleBlock,
    StandaloneStylesheet,
    InlineStyle,
    StaticAttrsOnly,
    BoundAttributes,
    TextInterpolation,
    BoundAttributeValue,
    ControlFlowTemplate,
    ContainsMarkupControl,
    RepeatControl,
    ConditionalControl,
    ComponentTag,
    SlotOutlet,
    ScriptStyleSeparated,
    CommonBoilerplateLike,
    HeaderFile,
    SourceFile,
    StaticLinkage,
    ExternLinkage,
    Inline,
    StaticInline,
    AbiFacing,
    LayoutContract,
    PreprocConditioned,
    SamePreprocCondition,
    DifferentPreprocCondition,
    MacroObjectLike,
    MacroFunctionLike,
    MacroVariadic,
    MacroTokenPaste,
    MacroStringify,
    IncludeGuard,
    PragmaOnce,
}

impl UnitEvidenceFlag {
    const ALL: [Self; 104] = [
        Self::SameSymbol,
        Self::SameOwner,
        Self::SameReceiverType,
        Self::SameSelfType,
        Self::SameTraitTarget,
        Self::SameExtensionTarget,
        Self::SameRootTag,
        Self::SameSelector,
        Self::DifferentSelectors,
        Self::SelectorExcludedFromProof,
        Self::HasRuntimeBody,
        Self::HasReusableBody,
        Self::DeclarationOnly,
        Self::TypeOnly,
        Self::RuntimeValue,
        Self::Ambient,
        Self::DeclarationFile,
        Self::StubFile,
        Self::EllipsisBody,
        Self::PassOnly,
        Self::AbstractOnly,
        Self::DecoratedBinding,
        Self::HasDefaultBody,
        Self::HasAssociatedType,
        Self::FieldOnly,
        Self::DataShapeOnly,
        Self::SameFieldSet,
        Self::SchemaLike,
        Self::RuntimeValidation,
        Self::ProtocolRequirement,
        Self::ProtocolExtension,
        Self::ConcreteTypeExtension,
        Self::ConstrainedExtension,
        Self::InterfaceDefaultMethod,
        Self::InterfaceStaticMethod,
        Self::InterfacePrivateMethod,
        Self::RecordHeader,
        Self::CompactConstructor,
        Self::EnumConstantBody,
        Self::AliasDeclaration,
        Self::DefinedType,
        Self::FieldTags,
        Self::ActorIsolated,
        Self::SwiftuiView,
        Self::ResultBuilderBody,
        Self::ReturnsView,
        Self::Async,
        Self::Throws,
        Self::TestContext,
        Self::TestFixture,
        Self::TestSuite,
        Self::TestCase,
        Self::TestHook,
        Self::AssertionDsl,
        Self::TableDrivenTest,
        Self::FrameworkHook,
        Self::RailsRouteDsl,
        Self::ActiveRecordValidation,
        Self::ActiveRecordScope,
        Self::MigrationDsl,
        Self::FactoryBotDsl,
        Self::RspecExample,
        Self::MinitestTest,
        Self::ComputedStyleEquivalent,
        Self::SameDeclarationBlock,
        Self::SameAtRuleContext,
        Self::AtRuleContext,
        Self::SingleDeclaration,
        Self::UtilityLikeSelector,
        Self::ResetLikeSelector,
        Self::CustomPropertyToken,
        Self::EmbeddedStyleBlock,
        Self::StandaloneStylesheet,
        Self::InlineStyle,
        Self::StaticAttrsOnly,
        Self::BoundAttributes,
        Self::TextInterpolation,
        Self::BoundAttributeValue,
        Self::ControlFlowTemplate,
        Self::ContainsMarkupControl,
        Self::RepeatControl,
        Self::ConditionalControl,
        Self::ComponentTag,
        Self::SlotOutlet,
        Self::ScriptStyleSeparated,
        Self::CommonBoilerplateLike,
        Self::HeaderFile,
        Self::SourceFile,
        Self::StaticLinkage,
        Self::ExternLinkage,
        Self::Inline,
        Self::StaticInline,
        Self::AbiFacing,
        Self::LayoutContract,
        Self::PreprocConditioned,
        Self::SamePreprocCondition,
        Self::DifferentPreprocCondition,
        Self::MacroObjectLike,
        Self::MacroFunctionLike,
        Self::MacroVariadic,
        Self::MacroTokenPaste,
        Self::MacroStringify,
        Self::IncludeGuard,
        Self::PragmaOnce,
    ];

    const fn bit(self) -> u128 {
        match self {
            Self::Unknown => 0,
            _ => 1u128 << (self as u8 - 1),
        }
    }
}
