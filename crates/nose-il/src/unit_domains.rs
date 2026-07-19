use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct UnitDomains(u16);

impl UnitDomains {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn of(domain: UnitDomain) -> Self {
        Self(domain.bit())
    }

    pub const fn with(self, domain: UnitDomain) -> Self {
        Self(self.0 | domain.bit())
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, domain: UnitDomain) -> bool {
        self.0 & domain.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = UnitDomain> {
        UnitDomain::ALL
            .iter()
            .copied()
            .filter(move |domain| self.contains(*domain))
    }
}

impl Serialize for UnitDomains {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for UnitDomains {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let domains = Vec::<UnitDomain>::deserialize(deserializer)?;
        Ok(domains
            .into_iter()
            .fold(UnitDomains::empty(), UnitDomains::with))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitDomain {
    Unknown,
    Imperative,
    TypeContract,
    ImplementationType,
    Data,
    Markup,
    Style,
    ModuleWiring,
    Preprocessor,
    Prose,
}

impl UnitDomain {
    const ALL: [Self; 9] = [
        Self::Imperative,
        Self::TypeContract,
        Self::ImplementationType,
        Self::Data,
        Self::Markup,
        Self::Style,
        Self::ModuleWiring,
        Self::Preprocessor,
        Self::Prose,
    ];

    const fn bit(self) -> u16 {
        match self {
            Self::Unknown => 0,
            Self::Imperative => 1 << 0,
            Self::TypeContract => 1 << 1,
            Self::ImplementationType => 1 << 2,
            Self::Data => 1 << 3,
            Self::Markup => 1 << 4,
            Self::Style => 1 << 5,
            Self::ModuleWiring => 1 << 6,
            Self::Preprocessor => 1 << 7,
            Self::Prose => 1 << 8,
        }
    }
}
