use semver::{Comparator, Op, Version, VersionReq};

#[derive(Clone)]
struct VersionInterval {
    lower: Option<(Version, bool)>,
    upper: Option<(Version, bool)>,
}

pub(super) fn requirements_may_overlap(left: &str, right: &str) -> bool {
    let Ok(left) = VersionReq::parse(&normalize_requirement(left)) else {
        return true;
    };
    let Ok(right) = VersionReq::parse(&normalize_requirement(right)) else {
        return true;
    };
    let (Some(left), Some(right)) = (requirement_interval(&left), requirement_interval(&right))
    else {
        return true;
    };
    interval_is_non_empty(&left)
        && interval_is_non_empty(&right)
        && intervals_overlap(&left, &right)
}

fn normalize_requirement(value: &str) -> String {
    value
        .replace(',', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(",")
}

fn requirement_interval(requirement: &VersionReq) -> Option<VersionInterval> {
    let mut interval = VersionInterval {
        lower: None,
        upper: None,
    };
    for comparator in &requirement.comparators {
        if !comparator.pre.is_empty() {
            return None;
        }
        let (lower, upper) = comparator_interval(comparator)?;
        if let Some(lower) = lower {
            interval.lower = stronger_lower(interval.lower.take(), lower);
        }
        if let Some(upper) = upper {
            interval.upper = stronger_upper(interval.upper.take(), upper);
        }
    }
    Some(interval)
}

type VersionBound = Option<(Version, bool)>;

fn comparator_interval(comparator: &Comparator) -> Option<(VersionBound, VersionBound)> {
    let floor = Version::new(
        comparator.major,
        comparator.minor.unwrap_or(0),
        comparator.patch.unwrap_or(0),
    );
    match comparator.op {
        Op::Exact => match (comparator.minor, comparator.patch) {
            (Some(_), Some(_)) => Some((Some((floor.clone(), true)), Some((floor, true)))),
            (Some(minor), None) => Some((
                Some((floor, true)),
                Some((
                    Version::new(comparator.major, minor.checked_add(1)?, 0),
                    false,
                )),
            )),
            (None, None) => Some((
                Some((floor, true)),
                Some((Version::new(comparator.major.checked_add(1)?, 0, 0), false)),
            )),
            (None, Some(_)) => None,
        },
        Op::Greater if comparator.patch.is_some() => Some((Some((floor, false)), None)),
        Op::GreaterEq if comparator.patch.is_some() => Some((Some((floor, true)), None)),
        Op::Less if comparator.patch.is_some() => Some((None, Some((floor, false)))),
        Op::LessEq if comparator.patch.is_some() => Some((None, Some((floor, true)))),
        Op::Tilde => {
            let upper = if let Some(minor) = comparator.minor {
                Version::new(comparator.major, minor.checked_add(1)?, 0)
            } else {
                Version::new(comparator.major.checked_add(1)?, 0, 0)
            };
            Some((Some((floor, true)), Some((upper, false))))
        }
        Op::Caret => {
            let upper = if comparator.major > 0 {
                Version::new(comparator.major.checked_add(1)?, 0, 0)
            } else if comparator.minor.unwrap_or(0) > 0 {
                Version::new(0, comparator.minor?.checked_add(1)?, 0)
            } else {
                Version::new(0, 0, comparator.patch.unwrap_or(0).checked_add(1)?)
            };
            Some((Some((floor, true)), Some((upper, false))))
        }
        Op::Wildcard => match comparator.minor {
            None => Some((
                Some((floor, true)),
                Some((Version::new(comparator.major.checked_add(1)?, 0, 0), false)),
            )),
            Some(minor) => Some((
                Some((floor, true)),
                Some((
                    Version::new(comparator.major, minor.checked_add(1)?, 0),
                    false,
                )),
            )),
        },
        Op::Greater | Op::GreaterEq | Op::Less | Op::LessEq => None,
        _ => None,
    }
}

fn stronger_lower(current: VersionBound, candidate: (Version, bool)) -> VersionBound {
    match current {
        None => Some(candidate),
        Some(current) => match current.0.cmp(&candidate.0) {
            std::cmp::Ordering::Less => Some(candidate),
            std::cmp::Ordering::Greater => Some(current),
            std::cmp::Ordering::Equal => Some((current.0, current.1 && candidate.1)),
        },
    }
}

fn stronger_upper(current: VersionBound, candidate: (Version, bool)) -> VersionBound {
    match current {
        None => Some(candidate),
        Some(current) => match current.0.cmp(&candidate.0) {
            std::cmp::Ordering::Greater => Some(candidate),
            std::cmp::Ordering::Less => Some(current),
            std::cmp::Ordering::Equal => Some((current.0, current.1 && candidate.1)),
        },
    }
}

fn interval_is_non_empty(interval: &VersionInterval) -> bool {
    match (&interval.lower, &interval.upper) {
        (Some((lower, lower_inclusive)), Some((upper, upper_inclusive))) => {
            lower < upper || (lower == upper && *lower_inclusive && *upper_inclusive)
        }
        _ => true,
    }
}

fn intervals_overlap(left: &VersionInterval, right: &VersionInterval) -> bool {
    let lower = stronger_lower(
        left.lower.clone(),
        right
            .lower
            .clone()
            .unwrap_or_else(|| (Version::new(0, 0, 0), true)),
    );
    let upper = match (&left.upper, &right.upper) {
        (Some(left), Some(right)) => stronger_upper(Some(left.clone()), right.clone()),
        (Some(left), None) => Some(left.clone()),
        (None, Some(right)) => Some(right.clone()),
        (None, None) => None,
    };
    interval_is_non_empty(&VersionInterval { lower, upper })
}

#[cfg(test)]
mod tests {
    use super::requirements_may_overlap;

    #[test]
    fn detects_overlap_and_proves_simple_disjoint_ranges() {
        assert!(requirements_may_overlap(">=1.0.0 <3.0.0", ">=2.0.0 <4.0.0"));
        assert!(!requirements_may_overlap(
            ">=1.0.0 <2.0.0",
            ">=2.0.0 <3.0.0"
        ));
        assert!(requirements_may_overlap("^1.2.0", ">=1.9.0 <2.0.0"));
        assert!(!requirements_may_overlap("^1.2.0", ">=2.0.0"));
    }
}
