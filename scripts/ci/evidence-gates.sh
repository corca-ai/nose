#!/usr/bin/env bash
# Domain-owned command batches behind check-ci-local.sh named evidence gates.

run_default_head_residual_ranking_selftest() {
    python3 bench/labels/residual_ranking.py self-test
}

run_default_head_residual_topup_selftest() {
    python3 bench/labels/residual_ranking_topup.py self-test
}

run_default_head_residual_panel_selftest() {
    python3 bench/labels/residual_ranking_panel.py self-test
}

run_default_head_residual_closeout_selftest() {
    python3 bench/labels/residual_ranking_closeout.py self-test
}

run_default_head_closeout_selftest() {
    python3 bench/labels/default_head_closeout.py --self-test
}

run_bounded_evidence_checks() {
    local max_jobs="$1"
    shift
    if [[ ! "$max_jobs" =~ ^[1-9][0-9]*$ ]]; then
        echo "evidence worker count must be a positive integer: $max_jobs" >&2
        return 2
    fi

    local log_dir
    log_dir="$(mktemp -d "${TMPDIR:-/tmp}/nose-evidence.XXXXXX")"
    local -a checks=("$@")
    local check_count="${#checks[@]}"
    local -a pids=()
    local -a logs=()
    local -a status_files=()
    local next_index=0
    local active_count=0
    local completed_count=0
    local first_status=0

    while [[ "$completed_count" -lt "$check_count" ]]; do
        while [[ "$active_count" -lt "$max_jobs" && "$next_index" -lt "$check_count" ]]; do
            local check_name="${checks[$next_index]}"
            local log_file="$log_dir/$next_index.log"
            local status_file="$log_dir/$next_index.status"
            (
                local status=0
                if "$check_name"; then
                    status=0
                else
                    status=$?
                fi
                printf '%s\n' "$status" >"$status_file"
            ) >"$log_file" 2>&1 &
            pids[next_index]="$!"
            logs[next_index]="$log_file"
            status_files[next_index]="$status_file"
            next_index=$((next_index + 1))
            active_count=$((active_count + 1))
        done

        local made_progress=0
        local running_index
        for ((running_index = 0; running_index < next_index; running_index++)); do
            if [[ -z "${pids[$running_index]:-}" || ! -s "${status_files[$running_index]}" ]]; then
                continue
            fi

            wait "${pids[$running_index]}"
            pids[running_index]=""
            active_count=$((active_count - 1))
            completed_count=$((completed_count + 1))
            made_progress=1
        done
        if [[ "$made_progress" -eq 0 ]]; then
            sleep 0.05
        fi
    done

    local result_index
    for ((result_index = 0; result_index < check_count; result_index++)); do
        local status
        status="$(<"${status_files[$result_index]}")"
        cat "${logs[$result_index]}"
        if [[ "$status" -ne 0 ]]; then
            echo "parallel evidence check failed: ${checks[$result_index]}" >&2
            if [[ "$first_status" -eq 0 ]]; then
                first_status="$status"
            fi
        fi
        rm -f -- "${logs[$result_index]}" "${status_files[$result_index]}"
    done

    rmdir -- "$log_dir"
    return "$first_status"
}

run_default_head_evidence_checks() {
    need_cmd python3
    need_cmd node
    python3 scripts/check-domain-calibration.py
    python3 scripts/check-domain-calibration.py --self-test
    python3 bench/labels/query_schema.py --self-test
    python3 bench/labels/default_head_query_schema.py --self-test
    python3 bench/labels/live_query_schema.py --self-test
    python3 bench/labels/eval_by_language.py --self-test
    python3 bench/labels/check_default_head_baseline.py --self-test
    python3 bench/labels/check_default_head_baseline.py
    python3 bench/labels/labelset.py --self-test
    python3 bench/labels/label_refresh.py --self-test
    python3 bench/labels/default_head_taxonomy.py --self-test
    python3 bench/labels/label_refresh.py validate-runway \
        --dev-candidates bench/labels/default_head_label_runway_2026_07_13.dev.v1.json \
        --heldout-seal bench/labels/default_head_label_runway_2026_07_13.heldout.seal.v1.json \
        --labelset bench/labels/refactoring_families.v7.json \
        --evaluation bench/labels/product_quality_evaluation_v7_dev_runway_2026_07_13.v1.json
    python3 bench/labels/default_head_taxonomy.py validate \
        bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json \
        --pragmatic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.pragmatic.v1.json \
        --dedupe bench/labels/default_head_taxonomy_votes_2026_07_13.dev.dedupe.v1.json \
        --skeptic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.skeptic.v1.json
    python3 bench/labels/default_head_heldout.py validate
    python3 bench/labels/default_head_heldout.py self-test
    python3 bench/labels/default_head_heldout_commitment_receipt.py validate
    python3 bench/labels/default_head_heldout_commitment_receipt.py self-test
    python3 bench/labels/default_head_heldout_panel.py self-test
    python3 bench/labels/default_head_heldout_vote_receipt.py validate
    python3 bench/labels/default_head_heldout_vote_receipt.py self-test
    python3 bench/labels/default_head_heldout_arbitration.py self-test
    python3 bench/labels/default_head_heldout_arbitration.py validate
    python3 bench/labels/default_head_heldout_arbitration_receipt.py validate
    python3 bench/labels/default_head_heldout_arbitration_receipt.py self-test
    python3 bench/labels/default_head_heldout_arbitration_result.py self-test
    python3 bench/labels/default_head_heldout_arbitration_result.py validate-public \
        bench/labels/default_head_heldout_arbitration_result_2026_07_14.heldout.v3.json
    python3 bench/labels/default_head_heldout_arbitration_result_receipt.py validate
    python3 bench/labels/default_head_heldout_arbitration_result_receipt.py self-test
    python3 bench/labels/default_head_heldout_reveal.py self-test
    test ! -e bench/labels/.default_head_heldout_reveal.transaction.json
    test ! -L bench/labels/.default_head_heldout_reveal.transaction.json
    local reveal=bench/labels/default_head_heldout_reveal_2026_07_14.heldout.v3.json
    if [[ -e "$reveal" || -L "$reveal" ]]; then
        python3 bench/labels/default_head_heldout_reveal.py validate
        python3 bench/labels/default_head_heldout_reveal_receipt.py validate
        python3 bench/labels/default_head_heldout_reveal_receipt.py self-test
    fi
    python3 bench/labels/proof_actionability_no_go.py --self-test
    python3 bench/labels/residual_ranking.py validate
    python3 bench/labels/residual_ranking_topup.py validate
    python3 bench/labels/residual_ranking_panel.py validate-arbitration
    python3 bench/labels/residual_ranking_panel.py validate-decisions
    python3 bench/labels/residual_ranking_panel.py validate-component
    python3 bench/labels/residual_ranking_closeout.py validate
    python3 bench/labels/default_head_fresh_repository_audit.py
    python3 bench/labels/default_head_fresh_repository_audit.py --self-test
    python3 bench/labels/default_head_measurement_replay.py validate
    python3 bench/labels/default_head_measurement_replay.py self-test
    python3 bench/labels/default_head_closeout.py
    run_bounded_evidence_checks "${NOSE_DEFAULT_HEAD_JOBS:-3}" \
        run_default_head_residual_ranking_selftest \
        run_default_head_residual_topup_selftest \
        run_default_head_residual_panel_selftest \
        run_default_head_residual_closeout_selftest \
        run_default_head_closeout_selftest
}

run_divergence_evidence_checks() {
    need_cmd python3
    python3 eval/divergence_fire/replay.py selftest
    python3 eval/divergence_fire/replay.py check-artifacts
    python3 eval/divergence_fire/precision_protocol.py validate
    python3 eval/divergence_fire/precision_protocol.py self-test
    python3 eval/divergence_fire/precision_protocol_receipt.py validate
    python3 eval/divergence_fire/precision_protocol_receipt.py self-test
}

run_surface_and_recall_evidence_checks() {
    need_cmd python3
    python3 bench/labels/generated_provenance_behavior.py --self-test
    python3 bench/labels/generated_provenance_behavior.py validate
    python3 bench/labels/generated_provenance_closeout.py --self-test
    python3 bench/labels/generated_provenance_closeout.py
    python3 bench/labels/declaration_type_contract_behavior.py --self-test
    python3 bench/labels/declaration_type_contract_behavior.py validate
    python3 bench/labels/declaration_type_contract_closeout.py --self-test
    python3 bench/labels/declaration_type_contract_closeout.py
    python3 bench/labels/recall_ceiling_probe.py --self-test
    python3 bench/labels/missed_worthy_stage_audit.py --self-test
    python3 bench/labels/missed_worthy_heldout_confirmation.py --self-test
    python3 bench/labels/missed_worthy_source_bounds.py --self-test
    python3 bench/labels/accepted_pair_coverage.py --self-test
}

run_runtime_and_soundness_evidence_checks() {
    need_cmd python3
    python3 scripts/binary_identity.py --self-test
    python3 scripts/query-regression-harness.py --self-test
    python3 scripts/cache-query-regression.py --self-test
    python3 scripts/cache-query-regression.py --validate-receipt \
        bench/cache/issue-872-mutation-matrix-receipt-2026-07-20.v1.json
    python3 scripts/cache-query-regression.py --validate-report \
        bench/cache/issue-872-v0.19.0-vs-candidate-sympy-paired-2026-07-20.v1.json
    python3 scripts/cache-query-regression.py --validate-report \
        bench/cache/issue-873-portable-cas-sympy-paired-2026-07-20.v1.json
    python3 scripts/watch-session-benchmark.py --self-test
    python3 scripts/watch-session-benchmark.py --validate-report \
        bench/cache/issue-878-watch-session-2026-07-21.v1.json
    python3 scripts/check-release-evidence-0.20.0.py --self-test
    python3 scripts/check-release-evidence-0.20.0.py
    python3 scripts/ruby-redefinition-scaling.py --self-test
    python3 scripts/semantic-regression-summary.py --self-test
    python3 scripts/recall-loss-diff.py --self-test
    python3 scripts/check-query-regression.py --self-test
    python3 scripts/check-recall-loss-baselines.py --self-test
    python3 scripts/check-soundness-scorecard.py --self-test
    python3 scripts/check-soundness-scorecard.py
    python3 scripts/soundness-lab-gate.py self-test
    python3 scripts/soundness-lab-gate.py check
    python3 scripts/soundness_exclusions.py --self-test
    python3 scripts/soundness_exclusions.py
}

run_accepted_pair_coverage_checks() {
    need_cmd python3
    python3 bench/labels/accepted_pair_coverage.py \
        --validate bench/labels/accepted_pair_coverage_2026_07_11.dev.baseline.v2.json
    python3 bench/labels/accepted_pair_coverage.py \
        --validate bench/labels/accepted_pair_coverage_2026_07_11.dev.head.v2.json
    python3 scripts/check-query-regression.py \
        bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.primary.v3.json \
        --same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.control.v3.json \
        --expected-drift-manifest bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.expected-drift.v1.json \
        --focused-report bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.focused.v3.json \
        --focused-same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.focused-control.v3.json \
        --require-same-binary-control \
        --max-runtime-delta-pct 5 \
        --min-runtime-delta-ms 5 \
        --check-status bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.status.v3.json \
        --check-markdown bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.summary.v3.md
    python3 scripts/check-query-regression.py \
        bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.primary.v3.json \
        --same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.control.v3.json \
        --expected-drift-manifest bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.expected-drift.v1.json \
        --focused-report bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.focused.v3.json \
        --focused-same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.focused-control.v3.json \
        --require-same-binary-control \
        --max-runtime-delta-pct 5 \
        --min-runtime-delta-ms 5 \
        --check-status bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.status.v3.json \
        --check-markdown bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.summary.v3.md
}

run_missed_worthy_frontier_checks() {
    need_cmd python3
    python3 bench/labels/recall_ceiling_probe.py \
        --validate bench/labels/recall_ceiling_probe_2026_07_11.v2.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_2026_07_11.dev.v1.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate-decisions bench/labels/missed_worthy_audit_decisions_2026_07_11.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_2026_07_11.v2.json
    python3 bench/labels/missed_worthy_heldout_confirmation.py \
        --validate bench/labels/missed_worthy_stage_confirmation_2026_07_11.heldout.v1.json
    python3 bench/labels/missed_worthy_source_bounds.py \
        --validate bench/labels/missed_worthy_audit_source_bounds_2026_07_11.dev.v1.json
    python3 scripts/check-query-regression.py \
        bench/labels/missed_worthy_grouping_pricing_2026_07_11.primary.v1.json \
        --same-binary-control bench/labels/missed_worthy_grouping_pricing_2026_07_11.control.v1.json \
        --require-same-binary-control \
        --max-runtime-delta-pct 5 \
        --min-runtime-delta-ms 5 \
        --check-status bench/labels/missed_worthy_grouping_pricing_2026_07_11.status.v1.json \
        --check-markdown bench/labels/missed_worthy_grouping_pricing_2026_07_11.summary.md
    python3 bench/labels/recall_ceiling_probe.py \
        --validate-closeout bench/labels/missed_worthy_frontier_closeout_2026_07_11.v1.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_post_817_2026_07_12.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate-decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
    python3 bench/labels/missed_worthy_source_bounds.py \
        --validate bench/labels/missed_worthy_audit_source_bounds_post_817_2026_07_12.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json \
        --decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json
    python3 bench/labels/missed_worthy_heldout_confirmation.py \
        --validate bench/labels/missed_worthy_stage_confirmation_post_817_2026_07_12.heldout.v2.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json \
        --decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_post_821_2026_07_13.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_issue_832_2026_07_13.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json
}
