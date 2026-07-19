use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitSubkind {
    #[default]
    Unknown,
    Function,
    Method,
    Constructor,
    FunctionPrototype,
    Class,
    Module,
    SingletonClass,
    SingletonMethod,
    StructRecord,
    RecordContract,
    Union,
    Enum,
    Actor,
    InterfaceTraitProtocol,
    TypeAlias,
    DefinedType,
    ExtensionImpl,
    ImplBlock,
    ObjectLiteral,
    ArrayLiteral,
    ConfigLiteral,
    CssRule,
    HtmlElement,
    MarkupFragment,
    MarkupControl,
    MarkdownSection,
    MarkdownDocument,
    DocMetadata,
    ImportIncludeReexport,
    Macro,
    Schema,
    DslBlock,
    TestDsl,
}

impl UnitSubkind {
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitBodyKind {
    #[default]
    Unknown,
    Implementation,
    DeclarationOnly,
    DeclarativeDenotation,
    ModuleWiring,
    Preprocessor,
    ProseContent,
    StructuredProse,
    Mixed,
}

impl UnitBodyKind {
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceGranularity {
    #[default]
    Unknown,
    WholeUnit,
    Member,
    ModuleItem,
    DeclarationGroup,
    Block,
    Fragment,
    Rule,
    Element,
    Section,
    Document,
    Mixed,
}

impl SourceGranularity {
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionKind {
    #[default]
    Unknown,
    Code,
    Script,
    Style,
    Markup,
    Preprocessor,
    Frontmatter,
    Prose,
    CodeFence,
    Mixed,
}

impl RegionKind {
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitContainerKind {
    #[default]
    Unknown,
    StandaloneFile,
    HtmlDocument,
    Jsx,
    Tsx,
    VueSfc,
    SvelteComponent,
}

impl UnitContainerKind {
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}
