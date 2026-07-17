use super::*;

fn independently_observes_mixed_exit(exits: &[nose_normalize::UnitExit]) -> bool {
    exits
        .iter()
        .any(|exit| *exit == nose_normalize::UnitExit::Fallthrough)
        && exits.iter().any(|exit| {
            matches!(
                exit,
                nose_normalize::UnitExit::Return | nose_normalize::UnitExit::Throw
            )
        })
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub(super) fn collect_product_fragment_verify_rec(
    n: &nose_il::Il,
    core: &nose_il::Il,
    interner: &Interner,
    battery: &[Vec<nose_normalize::Value>],
    oracle: &mut VerifyOracle,
    core_fragments: &std::collections::HashMap<
        (u32, u32, nose_detect::FragmentKind),
        Vec<nose_detect::FragmentContract>,
    >,
    file_idx: usize,
    fragment: nose_detect::ProductOracleFragment,
    tranche: OracleTranche,
) {
    let file_path = &n.meta.path;
    let span = n.node(fragment.root).span;
    let fp = fragment.value;
    let product_admission = fragment.product_admission.label();
    let claimable = fragment.product_admission.admitted()
        && nose_detect::exact_claim_eligible_parts(fragment.exact_safe, fp.len());
    let location = CensusLocation {
        unique: format!(
            "{}:{}-{}@{}-{}#{}",
            file_path,
            span.start_line,
            span.end_line,
            span.start_byte,
            span.end_byte,
            fragment.contract.kind.reason_code(),
        ),
        verify: format!(
            "{}:{}-{}@{}-{}",
            file_path, span.start_line, span.end_line, span.start_byte, span.end_byte
        ),
    };
    oracle.total += 1;

    let key = (span.start_byte, span.end_byte, fragment.contract.kind);
    let Some(core_contracts) = core_fragments.get(&key) else {
        let blocker = synthetic_blocker(
            "il",
            "il.fragment-core-span",
            fragment.contract.kind.reason_code(),
        );
        let outcome = census_outcome(
            "no-core-span",
            fragment.exact_safe,
            product_admission,
            claimable,
            None,
            Some(blocker),
        );
        push_verify_census(oracle, &location, n, fragment.root, &fp, outcome);
        oracle
            .exclusions
            .record_core_missing(file_path, span, fragment.token_count);
        return;
    };
    let [core_contract] = core_contracts.as_slice() else {
        // A span collision cannot identify one pre-canonical execution target. Do not choose an
        // arena node by traversal order; that would make the oracle depend on an accident.
        let blocker = synthetic_blocker(
            "il",
            "il.fragment-core-span-ambiguous",
            fragment.contract.kind.reason_code(),
        );
        let outcome = census_outcome(
            "no-core-span",
            fragment.exact_safe,
            product_admission,
            claimable,
            None,
            Some(blocker),
        );
        push_verify_census(oracle, &location, n, fragment.root, &fp, outcome);
        oracle
            .exclusions
            .record_core_missing(file_path, span, fragment.token_count);
        return;
    };

    if verify_battery_over_budget(fragment.token_count, battery.len()) {
        let blocker = synthetic_blocker(
            "budget",
            "budget.oracle-cost",
            fragment.contract.kind.reason_code(),
        );
        let outcome = census_outcome(
            "battery-bail",
            fragment.exact_safe,
            product_admission,
            claimable,
            None,
            Some(blocker),
        );
        push_verify_census(oracle, &location, core, core_contract.root, &fp, outcome);
        oracle
            .exclusions
            .record_battery_bail(file_path, span, fragment.token_count);
        return;
    }
    if fp.is_empty() {
        let blocker = synthetic_blocker(
            "value",
            "value.empty-fingerprint",
            fragment.contract.kind.reason_code(),
        );
        let outcome = census_outcome(
            "empty-fp",
            fragment.exact_safe,
            product_admission,
            claimable,
            None,
            Some(blocker),
        );
        push_verify_census(oracle, &location, n, fragment.root, &fp, outcome);
        oracle
            .exclusions
            .record_empty_fingerprint(file_path, span, fragment.token_count);
        return;
    }
    let Some(contracts) = fragment.oracle_contracts.as_deref() else {
        record_fragment_oracle_exclusion(
            oracle,
            &location,
            core,
            core_contract.root,
            &fp,
            fragment.exact_safe,
            product_admission,
            claimable,
            file_path,
            span,
            fragment.token_count,
            synthetic_blocker(
                "contract",
                "oracle.fragment-contract-coordinate",
                fragment.contract.kind.reason_code(),
            ),
        );
        return;
    };
    let Some((core_wrapper, core_root)) = nose_detect::synthesize_wrapper_with_module_strings(
        core,
        interner,
        core_contract,
        tranche.includes_swift_module_strings(),
    ) else {
        record_fragment_oracle_exclusion(
            oracle,
            &location,
            core,
            core_contract.root,
            &fp,
            fragment.exact_safe,
            product_admission,
            claimable,
            file_path,
            span,
            fragment.token_count,
            synthetic_blocker(
                "contract",
                "oracle.fragment-wrapper-synthesis",
                fragment.contract.kind.reason_code(),
            ),
        );
        return;
    };
    let Some((full_wrapper, full_root)) = nose_detect::synthesize_wrapper_with_module_strings(
        n,
        interner,
        &fragment.contract,
        tranche.includes_swift_module_strings(),
    ) else {
        record_fragment_oracle_exclusion(
            oracle,
            &location,
            core,
            core_contract.root,
            &fp,
            fragment.exact_safe,
            product_admission,
            claimable,
            file_path,
            span,
            fragment.token_count,
            synthetic_blocker(
                "contract",
                "oracle.fragment-wrapper-synthesis",
                fragment.contract.kind.reason_code(),
            ),
        );
        return;
    };

    let input_projections = if tranche.includes_cardinality() {
        nose_detect::fragment_input_projections(n, &fragment.contract)
    } else {
        vec![nose_detect::OracleInputProjection::Declared; fragment.contract.inputs.len()]
    };
    let core_input_projections = if tranche.includes_cardinality() {
        nose_detect::fragment_input_projections(core, core_contract)
    } else {
        vec![nose_detect::OracleInputProjection::Declared; core_contract.inputs.len()]
    };
    if input_projections != core_input_projections {
        record_fragment_oracle_exclusion(
            oracle,
            &location,
            core,
            core_contract.root,
            &fp,
            fragment.exact_safe,
            product_admission,
            claimable,
            file_path,
            span,
            fragment.token_count,
            synthetic_blocker(
                "contract",
                "oracle.fragment-input-projection-drift",
                fragment.contract.kind.reason_code(),
            ),
        );
        return;
    }
    let product_observes_mixed_exit = nose_detect::fragment_observes_mixed_exit(
        n,
        fragment.contract.root,
        fragment.contract.kind,
    );
    if product_observes_mixed_exit
        != nose_detect::fragment_observes_mixed_exit(core, core_contract.root, core_contract.kind)
    {
        record_fragment_oracle_exclusion(
            oracle,
            &location,
            core,
            core_contract.root,
            &fp,
            fragment.exact_safe,
            product_admission,
            claimable,
            file_path,
            span,
            fragment.token_count,
            synthetic_blocker(
                "contract",
                "oracle.fragment-control-observation-drift",
                fragment.contract.kind.reason_code(),
            ),
        );
        return;
    }
    let (beh, fragment_exits) = match run_fragment_battery_diagnostic_with_oracle_proofs(
        &core_wrapper,
        interner,
        core_root,
        battery,
        contracts,
        tranche.includes_swift_module_strings(),
    ) {
        Ok(result) => result,
        Err(blocker) => {
            record_fragment_oracle_exclusion(
                oracle,
                &location,
                core,
                core_contract.root,
                &fp,
                fragment.exact_safe,
                product_admission,
                claimable,
                file_path,
                span,
                fragment.token_count,
                blocker,
            );
            return;
        }
    };
    let outcome = census_outcome(
        "interpretable",
        fragment.exact_safe,
        product_admission,
        claimable,
        None,
        None,
    );
    push_verify_census(oracle, &location, core, core_contract.root, &fp, outcome);

    // Keep the audit oracle independent from the product's structural mixed-exit classifier.
    // If a fingerprint-tag regression makes a mixed fragment collide with a whole function,
    // the executed battery must still retain the terminal-control distinction and report it.
    let oracle_observes_mixed_exit = independently_observes_mixed_exit(&fragment_exits);

    let mut canon_exposed = false;
    if let Ok((full_beh, full_exits)) = run_fragment_battery_diagnostic_with_oracle_proofs(
        &full_wrapper,
        interner,
        full_root,
        battery,
        contracts,
        tranche.includes_swift_module_strings(),
    ) {
        let concrete = !beh.iter().any(nose_normalize::behavior_has_sym)
            && !full_beh.iter().any(nose_normalize::behavior_has_sym);
        if concrete {
            canon_exposed = true;
            oracle.canon_checked += 1;
            if (canon_changed_behavior(&beh, &full_beh)
                || (oracle_observes_mixed_exit && fragment_exits != full_exits))
                && oracle.canon_violations.len() < 20
            {
                oracle.canon_violations.push(format!(
                    "{}:{}-{}#{}",
                    file_path,
                    span.start_line,
                    span.end_line,
                    fragment.contract.kind.reason_code(),
                ));
            }
        }
    }
    let param_domains = param_domains(&full_wrapper, full_root);
    oracle.recs.push(VerifyRec {
        lang: n.meta.lang,
        fp,
        beh,
        file: file_path.to_string(),
        start: span.start_line,
        end: span.end_line,
        tokens: fragment.token_count,
        loc: location.verify,
        claimable,
        product_admission,
        canon_exposed,
        admission_rejection: None,
        domain_sig: param_domain_signature(&param_domains, &input_projections),
        param_domains,
        input_projections,
        file_idx,
        core_root: core_contract.root,
        core_fragment: Some(core_contract.clone()),
        fragment_exits: oracle_observes_mixed_exit.then_some(fragment_exits),
    });
}

#[cfg(test)]
mod tests {
    use super::independently_observes_mixed_exit;
    use nose_normalize::UnitExit;

    #[test]
    fn oracle_mixed_exit_requires_fallthrough_and_terminal_control() {
        assert!(independently_observes_mixed_exit(&[
            UnitExit::Fallthrough,
            UnitExit::Return,
        ]));
        assert!(!independently_observes_mixed_exit(&[
            UnitExit::Return,
            UnitExit::Error,
        ]));
        assert!(independently_observes_mixed_exit(&[
            UnitExit::Fallthrough,
            UnitExit::Throw,
        ]));
        assert!(!independently_observes_mixed_exit(&[
            UnitExit::Fallthrough,
            UnitExit::Fallthrough,
        ]));
    }
}

#[allow(clippy::too_many_arguments)]
fn record_fragment_oracle_exclusion(
    oracle: &mut VerifyOracle,
    location: &CensusLocation,
    core: &nose_il::Il,
    core_root: nose_il::NodeId,
    fp: &[u64],
    exact_safe: bool,
    product_admission: &'static str,
    claimable: bool,
    file_path: &str,
    span: nose_il::Span,
    tokens: usize,
    blocker: nose_normalize::InterpreterBlocker,
) {
    let path_cap = blocker.capability_id == "budget.symbolic-branch-sites";
    let (reason, exclusion) = if path_cap {
        ("path-bail", VerifyExclusionReason::PathBail)
    } else {
        ("battery-bail", VerifyExclusionReason::Uninterpretable)
    };
    let outcome = census_outcome(
        reason,
        exact_safe,
        product_admission,
        claimable,
        None,
        Some(blocker),
    );
    push_verify_census(oracle, location, core, core_root, fp, outcome);
    oracle
        .exclusions
        .record(exclusion, file_path, span, tokens, None);
}
