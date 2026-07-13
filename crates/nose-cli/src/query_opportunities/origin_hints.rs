struct OriginFactSummary {
    all_style: bool,
    all_markup: bool,
    all_preprocessor: bool,
    all_type_contract: bool,
    any_implementation_type: bool,
    all_implementation_type: bool,
    all_imperative: bool,
    all_interface_trait_protocol: bool,
    all_class: bool,
    all_declaration_only: bool,
    all_declarative_denotation: bool,
    any_mixed_body: bool,
    any_implementation_body: bool,
    every_named_copy_same_symbol: bool,
}

impl OriginFactSummary {
    fn from_family(f: &nose_detect::RefactorFamily) -> Option<Self> {
        use nose_il::{UnitBodyKind, UnitDomain, UnitSubkind};

        if f.locations.iter().all(|loc| loc.origin.is_unknown()) {
            return None;
        }

        let all_have_domain = |domain| f.locations.iter().all(|loc| loc.origin.has_domain(domain));
        let any_have_domain = |domain| f.locations.iter().any(|loc| loc.origin.has_domain(domain));
        let all_subkind = |subkind| f.locations.iter().all(|loc| loc.origin.subkind == subkind);
        let all_body = |body_kind| {
            f.locations
                .iter()
                .all(|loc| loc.origin.body_kind == body_kind)
        };
        let any_body = |body_kind| {
            f.locations
                .iter()
                .any(|loc| loc.origin.body_kind == body_kind)
        };
        let mut names = f.locations.iter().filter_map(|loc| loc.name.as_deref());
        let every_named_copy_same_symbol = if let Some(first) = names.next() {
            f.locations.iter().filter(|loc| loc.name.is_some()).count() == f.members
                && names.all(|name| name == first)
        } else {
            false
        };

        Some(Self {
            all_style: all_have_domain(UnitDomain::Style),
            all_markup: all_have_domain(UnitDomain::Markup),
            all_preprocessor: all_have_domain(UnitDomain::Preprocessor),
            all_type_contract: all_have_domain(UnitDomain::TypeContract),
            any_implementation_type: any_have_domain(UnitDomain::ImplementationType),
            all_implementation_type: all_have_domain(UnitDomain::ImplementationType),
            all_imperative: all_have_domain(UnitDomain::Imperative),
            all_interface_trait_protocol: all_subkind(UnitSubkind::InterfaceTraitProtocol),
            all_class: all_subkind(UnitSubkind::Class),
            all_declaration_only: all_body(UnitBodyKind::DeclarationOnly),
            all_declarative_denotation: all_body(UnitBodyKind::DeclarativeDenotation),
            any_mixed_body: any_body(UnitBodyKind::Mixed),
            any_implementation_body: any_body(UnitBodyKind::Implementation),
            every_named_copy_same_symbol,
        })
    }
}

pub(super) fn origin_extract_hint(f: &nose_detect::RefactorFamily) -> Option<&'static str> {
    let facts = OriginFactSummary::from_family(f)?;

    if facts.all_style {
        return Some(
            "merge selectors or move the declarations to a shared class/token if these elements should be coupled",
        );
    }
    if facts.all_markup {
        return Some("share a component/template only if the data shape matches");
    }
    if facts.all_preprocessor {
        return Some("divergence macro expansion and conditional context before sharing");
    }
    if facts.all_type_contract && !facts.any_implementation_type {
        if facts.all_interface_trait_protocol {
            return Some("consolidate one shared interface/protocol contract");
        }
        return Some("consolidate one shared type/API contract");
    }
    if facts.all_type_contract && facts.any_implementation_type {
        return Some(
            "consolidate the type contract; divergence whether shared behavior should move too",
        );
    }
    if facts.all_implementation_type {
        if facts.all_class && (facts.any_implementation_body || facts.any_mixed_body) {
            return Some("extract a shared base class / mixin");
        }
        return Some("consolidate shared type implementation");
    }
    if facts.all_imperative {
        return Some("extract a helper");
    }
    None
}

pub(crate) fn proposal_action_label(f: &nose_detect::RefactorFamily) -> &'static str {
    use nose_il::UnitKind;

    if let Some(origin_hint) = origin_extract_hint(f) {
        return match origin_hint {
            "extract a helper" => "extract a shared helper",
            other => other,
        };
    }
    let all_classes = f.locations.iter().all(|loc| loc.kind == UnitKind::Class);
    let all_blocks = f.locations.iter().all(|loc| loc.kind == UnitKind::Block);
    let type_decl = all_classes && f.mean_sem < 12.0;
    if type_decl {
        "consolidate into one shared type"
    } else if all_classes {
        "extract a shared base class / mixin"
    } else if all_blocks {
        "extract a method from the repeated block"
    } else {
        "extract a shared helper"
    }
}

pub(crate) fn hint_reasons(f: &nose_detect::RefactorFamily) -> Vec<String> {
    let Some(facts) = OriginFactSummary::from_family(f) else {
        return Vec::new();
    };

    let mut reasons = Vec::new();
    if facts.all_type_contract {
        if facts.all_interface_trait_protocol {
            reasons.push(format!(
                "all copies are {} interface/protocol contracts",
                family_language_label(f)
            ));
        } else {
            reasons.push("all copies are type/API contract regions".to_string());
        }
    } else if facts.all_implementation_type {
        reasons.push("all copies are behavior-bearing type implementation regions".to_string());
    } else if facts.all_style {
        reasons.push("all copies are declarative style rules".to_string());
    } else if facts.all_markup {
        reasons.push("all copies are rendered markup/template regions".to_string());
    } else if facts.all_preprocessor {
        reasons.push("all copies are macro/preprocessor regions".to_string());
    } else if facts.all_imperative {
        reasons.push("all copies are imperative callable regions".to_string());
    }

    if facts.all_declaration_only {
        reasons.push("no implementation body was found".to_string());
    } else if facts.all_declarative_denotation {
        reasons
            .push("the duplicate is a declaration/denotation, not an imperative body".to_string());
    } else if facts.any_mixed_body {
        reasons.push("some copied regions mix declarations with reusable behavior".to_string());
    } else if facts.any_implementation_body {
        reasons.push("an implementation body was found".to_string());
    }

    if facts.every_named_copy_same_symbol {
        reasons.push("every copy has the same symbol name".to_string());
    }
    reasons
}

fn family_language_label(f: &nose_detect::RefactorFamily) -> String {
    let mut langs = f
        .locations
        .iter()
        .map(|loc| loc.lang.as_str())
        .collect::<Vec<_>>();
    langs.sort_unstable();
    langs.dedup();
    if langs.len() == 1 {
        language_label(langs[0]).to_string()
    } else {
        "cross-language".to_string()
    }
}

fn language_label(lang: &str) -> &'static str {
    match lang {
        "css" => "CSS",
        "go" => "Go",
        "html" => "HTML",
        "javascript" => "JavaScript",
        "typescript" => "TypeScript",
        "rust" => "Rust",
        "swift" => "Swift",
        "java" => "Java",
        "python" => "Python",
        "ruby" => "Ruby",
        "c" => "C",
        "vue" => "Vue",
        "svelte" => "Svelte",
        _ => "same-language",
    }
}
