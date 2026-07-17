#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OracleTranche {
    CandidateFragmentAudit,
    FragmentCardinalityProjection,
    ImmutableSwiftModuleString,
    UnusedTrailingParameters,
}

impl OracleTranche {
    pub(crate) fn parse(value: Option<&str>) -> anyhow::Result<Self> {
        match value {
            None | Some("unused-trailing-parameters") => Ok(Self::UnusedTrailingParameters),
            Some("candidate-fragment-audit") => Ok(Self::CandidateFragmentAudit),
            Some("fragment-cardinality-projection") => Ok(Self::FragmentCardinalityProjection),
            Some("immutable-swift-module-string") => Ok(Self::ImmutableSwiftModuleString),
            Some(value) => anyhow::bail!("unknown Soundness Lab oracle tranche: {value}"),
        }
    }

    pub(crate) fn includes_cardinality(self) -> bool {
        self != Self::CandidateFragmentAudit
    }

    pub(crate) fn includes_swift_module_strings(self) -> bool {
        matches!(
            self,
            Self::ImmutableSwiftModuleString | Self::UnusedTrailingParameters
        )
    }

    pub(crate) fn includes_unused_trailing_parameters(self) -> bool {
        self == Self::UnusedTrailingParameters
    }
}

#[cfg(test)]
mod tests {
    use super::OracleTranche;

    #[test]
    fn tranches_are_cumulative_and_unknown_names_fail_closed() {
        use OracleTranche::{
            CandidateFragmentAudit, FragmentCardinalityProjection, ImmutableSwiftModuleString,
            UnusedTrailingParameters,
        };

        let stages = [
            ("candidate-fragment-audit", CandidateFragmentAudit),
            (
                "fragment-cardinality-projection",
                FragmentCardinalityProjection,
            ),
            ("immutable-swift-module-string", ImmutableSwiftModuleString),
            ("unused-trailing-parameters", UnusedTrailingParameters),
        ];
        for (name, expected) in stages {
            assert_eq!(OracleTranche::parse(Some(name)).unwrap(), expected);
        }
        assert_eq!(
            OracleTranche::parse(None).unwrap(),
            UnusedTrailingParameters
        );
        assert!(!CandidateFragmentAudit.includes_cardinality());
        assert!(FragmentCardinalityProjection.includes_cardinality());
        assert!(!FragmentCardinalityProjection.includes_swift_module_strings());
        assert!(ImmutableSwiftModuleString.includes_swift_module_strings());
        assert!(UnusedTrailingParameters.includes_swift_module_strings());
        assert!(!ImmutableSwiftModuleString.includes_unused_trailing_parameters());
        assert!(UnusedTrailingParameters.includes_unused_trailing_parameters());
        assert!(OracleTranche::parse(Some("future-unreviewed-proof")).is_err());
    }
}
