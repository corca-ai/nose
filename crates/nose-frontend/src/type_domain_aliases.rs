use nose_il::{DomainEvidence, EvidenceId};

#[derive(Clone, Copy)]
pub(crate) struct TypeDomainEvidenceProvenance {
    pub pack_id: &'static str,
    pub rule: &'static str,
}

#[derive(Clone)]
pub(crate) struct ResolvedTypeDomain {
    pub domain: DomainEvidence,
    pub dependencies: Vec<EvidenceId>,
    pub provenance: TypeDomainEvidenceProvenance,
}

pub(crate) struct TypeDomainAlias {
    pub alias: String,
    pub domain: DomainEvidence,
    pub evidence: Option<EvidenceId>,
    pub provenance: TypeDomainEvidenceProvenance,
}

#[derive(Default)]
pub(crate) struct TypeDomainAliases {
    aliases: Vec<TypeDomainAlias>,
}

impl TypeDomainAliases {
    pub(crate) fn record_normalized(
        &mut self,
        local: &str,
        domain: DomainEvidence,
        evidence: Option<EvidenceId>,
        provenance: TypeDomainEvidenceProvenance,
    ) {
        self.record_inner(normalize_type_text(local), domain, evidence, provenance);
    }

    pub(crate) fn record_exact(
        &mut self,
        local: &str,
        domain: DomainEvidence,
        evidence: Option<EvidenceId>,
        provenance: TypeDomainEvidenceProvenance,
    ) {
        self.record_inner(local.trim().to_string(), domain, evidence, provenance);
    }

    pub(crate) fn clear_normalized(&mut self, local: &str) {
        let alias = normalize_type_text(local);
        if alias.is_empty() {
            return;
        }
        self.aliases.retain(|known| known.alias != alias);
    }

    pub(crate) fn resolve_text(&self, text: &str) -> Option<ResolvedTypeDomain> {
        let t = normalize_type_text(text);
        self.aliases.iter().find_map(|known| {
            (t.contains(&format!(":{}[", known.alias)) || t.contains(&format!(":{}<", known.alias)))
                .then(|| ResolvedTypeDomain {
                    domain: known.domain,
                    dependencies: known.evidence.into_iter().collect(),
                    provenance: known.provenance,
                })
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &TypeDomainAlias> {
        self.aliases.iter()
    }

    fn record_inner(
        &mut self,
        alias: String,
        domain: DomainEvidence,
        evidence: Option<EvidenceId>,
        provenance: TypeDomainEvidenceProvenance,
    ) {
        if alias.is_empty() {
            return;
        }
        if let Some(existing) = self.aliases.iter_mut().find(|known| known.alias == alias) {
            existing.domain = domain;
            existing.evidence = evidence;
            existing.provenance = provenance;
            return;
        }
        self.aliases.push(TypeDomainAlias {
            alias,
            domain,
            evidence,
            provenance,
        });
    }
}

fn normalize_type_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}
