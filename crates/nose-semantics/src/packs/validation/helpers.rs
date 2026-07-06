use super::ALLOWED_REQUIREMENT_PREFIXES;

pub(super) fn require_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{label}` must be a non-empty string"));
    }
    Ok(())
}

pub(super) fn optional_non_empty(label: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        return Err(format!("`{label}` must be a non-empty string when present"));
    }
    Ok(())
}

pub(super) fn validate_nose_version_requirement(label: &str, value: &str) -> Result<(), String> {
    require_non_empty(label, value)?;
    let normalized = normalize_nose_version_requirement(value);
    let requirement = semver::VersionReq::parse(&normalized).map_err(|err| {
        format!("`{label}` contains unsupported version requirement `{value}`: {err}")
    })?;
    let current_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|err| format!("current nose package version is not semver-compatible: {err}"))?;
    if !requirement.matches(&current_version) {
        return Err(format!(
            "`{label}` range `{value}` does not include this nose binary version `{current_version}`"
        ));
    }
    Ok(())
}

fn normalize_nose_version_requirement(value: &str) -> String {
    value
        .split(',')
        .flat_map(normalize_version_constraints)
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalize_version_constraints(part: &str) -> Vec<String> {
    let mut constraints = Vec::new();
    let mut tokens = part.split_whitespace();
    while let Some(token) = tokens.next() {
        if is_version_operator(token) {
            let Some(version) = tokens.next() else {
                constraints.push(token.to_string());
                continue;
            };
            constraints.push(format!("{token}{}", strip_version_prefix(version)));
        } else {
            constraints.push(normalize_version_constraint(token));
        }
    }
    constraints
}

fn is_version_operator(value: &str) -> bool {
    matches!(value, ">=" | "<=" | ">" | "<" | "=" | "^" | "~")
}

fn strip_version_prefix(value: &str) -> &str {
    value.strip_prefix('v').unwrap_or(value)
}

fn normalize_version_constraint(constraint: &str) -> String {
    let (operator, version) = constraint
        .strip_prefix(">=")
        .map(|version| (">=", version))
        .or_else(|| constraint.strip_prefix("<=").map(|version| ("<=", version)))
        .or_else(|| constraint.strip_prefix('>').map(|version| (">", version)))
        .or_else(|| constraint.strip_prefix('<').map(|version| ("<", version)))
        .or_else(|| constraint.strip_prefix('=').map(|version| ("=", version)))
        .or_else(|| constraint.strip_prefix('^').map(|version| ("^", version)))
        .or_else(|| constraint.strip_prefix('~').map(|version| ("~", version)))
        .unwrap_or(("", constraint));
    let version = strip_version_prefix(version);
    format!("{operator}{version}")
}

pub(super) fn require_stable_id(label: &str, value: &str) -> Result<(), String> {
    require_non_empty(label, value)?;
    let mut chars = value.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
    {
        return Err(format!("`{label}` has invalid stable id `{value}`"));
    }
    Ok(())
}

pub(super) trait EvidenceKindPrefix {
    fn starts_with_evidence_prefix(&self) -> bool;
}

impl EvidenceKindPrefix for str {
    fn starts_with_evidence_prefix(&self) -> bool {
        ALLOWED_REQUIREMENT_PREFIXES[..ALLOWED_REQUIREMENT_PREFIXES.len() - 1]
            .iter()
            .any(|prefix| self.starts_with(prefix))
    }
}

pub(super) fn is_valid_evidence_kind(value: &str) -> bool {
    let Some(prefix) = ALLOWED_REQUIREMENT_PREFIXES[..ALLOWED_REQUIREMENT_PREFIXES.len() - 1]
        .iter()
        .find(|prefix| value.starts_with(**prefix))
    else {
        return false;
    };
    let suffix = &value[prefix.len()..];
    !suffix.is_empty()
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}
