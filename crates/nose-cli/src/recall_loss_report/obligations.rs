pub(super) fn rejection_obligation(
    reason: &'static str,
    missing_evidence: &[&'static str],
) -> (&'static str, &'static str) {
    let first_missing = missing_evidence
        .first()
        .copied()
        .unwrap_or("unknown-obligation-proof");
    match reason {
        "hof-demand-effect-proof-missing" => (
            "callback-demand-effect",
            hof_demand_effect_obligation_subreason(missing_evidence),
        ),
        "mutation-effect-boundary" => ("receiver-mutation", "effect-preserving-contract-missing"),
        "unsupported-runtime-boundary" => runtime_boundary_obligation(missing_evidence),
        "receiver-domain-proof-missing" => (
            "ambiguous-selector-boundary",
            "receiver-domain-proof-missing",
        ),
        "library-api-occurrence-proof-missing" => (
            "ambiguous-selector-boundary",
            "library-api-occurrence-evidence-missing",
        ),
        "source-surface-proof-missing" => match first_missing {
            "rust-macro-expansion-contract" => (
                "source-protocol-boundary",
                "rust-macro-expansion-contract-missing",
            ),
            _ => (
                "source-protocol-boundary",
                "source-surface-contract-missing",
            ),
        },
        "import-symbol-callee-identity-proof-missing" => {
            ("ambiguous-selector-boundary", first_missing)
        }
        "value-fingerprint-too-small" => (
            "non-degenerate-fingerprint-floor",
            "non-degenerate-value-fingerprint",
        ),
        "unattributed-strict-exact-unsafe" => {
            ("unattributed-boundary", "strict-exact-safe-tree-missing")
        }
        _ => ("unattributed-boundary", first_missing),
    }
}
type Obligation = (&'static str, &'static str);

struct RuntimeBoundaryRule {
    evidence: &'static str,
    obligation: Obligation,
}

const DEFAULT_RUNTIME_BOUNDARY_OBLIGATION: Obligation = (
    "scheduling-boundary",
    "runtime-protocol-boundary-contract-missing",
);

macro_rules! runtime_rule {
    ($evidence:literal => $family:literal, $subreason:literal) => {
        RuntimeBoundaryRule {
            evidence: $evidence,
            obligation: ($family, $subreason),
        }
    };
}

const RUNTIME_BOUNDARY_OBLIGATIONS: &[RuntimeBoundaryRule] = &[
    runtime_rule!("async-iteration-lifecycle-contract" => "lifecycle-materialization-boundary", "async-iteration-lifecycle-contract-missing"),
    runtime_rule!("async-iteration-value-channel-contract" => "success-error-result-channel", "async-iteration-value-channel-contract-missing"),
    runtime_rule!("async-context-lifecycle-contract" => "lifecycle-materialization-boundary", "async-context-lifecycle-contract-missing"),
    runtime_rule!("async-context-cleanup-contract" => "lifecycle-materialization-boundary", "async-context-cleanup-contract-missing"),
    runtime_rule!("async-await-scheduling-contract" => "scheduling-boundary", "async-await-scheduling-contract-missing"),
    runtime_rule!("promise-await-scheduling-contract" => "scheduling-boundary", "promise-await-scheduling-contract-missing"),
    runtime_rule!("promise-async-function-return-producer-proof" => "scheduling-boundary", "promise-async-function-return-producer-proof-missing"),
    runtime_rule!("async-function-scheduling-contract" => "scheduling-boundary", "async-function-scheduling-contract-missing"),
    runtime_rule!("promise-async-function-scheduling-contract" => "scheduling-boundary", "promise-async-function-scheduling-contract-missing"),
    runtime_rule!("async-block-scheduling-contract" => "scheduling-boundary", "async-block-scheduling-contract-missing"),
    runtime_rule!("future-async-block-scheduling-contract" => "scheduling-boundary", "future-async-block-scheduling-contract-missing"),
    runtime_rule!("promise-constructor-receiver-producer-proof" => "success-error-result-channel", "promise-constructor-receiver-producer-proof-missing"),
    RuntimeBoundaryRule {
        evidence: "promise-executor-timing-contract",
        obligation: (
            "executor-callback",
            "promise-executor-timing-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-executor-resolve-reject-callback-contract",
        obligation: (
            "executor-callback",
            "promise-executor-resolve-reject-callback-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-executor-throw-to-rejection-contract",
        obligation: (
            "rejection-channel",
            "promise-executor-throw-to-rejection-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-executor-callback-effect-contract",
        obligation: (
            "executor-callback",
            "promise-executor-callback-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-direct-function-return-domain-proof",
        obligation: (
            "success-error-result-channel",
            "promise-call-return-direct-function-return-domain-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-direct-method-return-domain-proof",
        obligation: (
            "success-error-result-channel",
            "promise-call-return-direct-method-return-domain-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-imported-function-settled-value-contract",
        obligation: (
            "success-error-result-channel",
            "promise-call-return-imported-function-settled-value-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-imported-member-settled-value-contract",
        obligation: (
            "success-error-result-channel",
            "promise-call-return-imported-member-settled-value-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-dynamic-dispatch-return-domain-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-dynamic-dispatch-return-domain-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-rejected-call-target-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-rejected-call-target-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-scoped-path-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-scoped-path-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-local-or-parameter-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-local-or-parameter-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-imported-binding-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-imported-binding-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-imported-member-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-imported-member-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-qualified-global-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-qualified-global-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-unshadowed-global-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-unshadowed-global-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-member-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-member-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-unknown-callee-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-unknown-callee-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-call-return-receiver-producer-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-call-return-receiver-producer-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-then-promise-like-receiver-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-then-promise-like-receiver-proof-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-then-fulfillment-continuation-contract",
        obligation: (
            "success-error-result-channel",
            "promise-then-fulfillment-continuation-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-then-rejection-continuation-contract",
        obligation: (
            "rejection-channel",
            "promise-then-rejection-continuation-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-then-callback-demand-effect-contract",
        obligation: (
            "callback-demand-effect",
            "promise-then-callback-demand-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-catch-rejection-continuation-contract",
        obligation: (
            "rejection-channel",
            "promise-catch-rejection-continuation-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-finally-settlement-continuation-contract",
        obligation: (
            "rejection-channel",
            "promise-finally-settlement-continuation-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-reject-rejected-value-channel-contract",
        obligation: (
            "rejection-channel",
            "promise-reject-rejected-value-channel-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-catch-callback-demand-effect-contract",
        obligation: (
            "callback-demand-effect",
            "promise-catch-callback-demand-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-finally-callback-demand-effect-contract",
        obligation: (
            "callback-demand-effect",
            "promise-finally-callback-demand-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-rejection-continuation-contract",
        obligation: (
            "rejection-channel",
            "promise-rejection-continuation-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-rejection-channel-contract",
        obligation: (
            "rejection-channel",
            "promise-rejection-channel-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-all-fulfilled-contract",
        obligation: (
            "success-error-result-channel",
            "promise-aggregate-all-fulfilled-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-ordered-values-contract",
        obligation: (
            "success-error-result-channel",
            "promise-aggregate-ordered-values-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-first-settled-contract",
        obligation: (
            "cancellation-liveness-boundary",
            "promise-aggregate-first-settled-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-cancellation-liveness-contract",
        obligation: (
            "cancellation-liveness-boundary",
            "promise-aggregate-cancellation-liveness-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-all-settled-contract",
        obligation: (
            "success-error-result-channel",
            "promise-aggregate-all-settled-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-settled-record-shape-contract",
        obligation: (
            "success-error-result-channel",
            "promise-aggregate-settled-record-shape-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-first-fulfilled-contract",
        obligation: (
            "success-error-result-channel",
            "promise-aggregate-first-fulfilled-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-error-channel-contract",
        obligation: (
            "rejection-channel",
            "promise-aggregate-error-channel-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-aggregate-result-channel-contract",
        obligation: (
            "success-error-result-channel",
            "promise-aggregate-result-channel-contract-missing",
        ),
    },
    runtime_rule!("abort-signal-cancellation-contract" => "cancellation-liveness-boundary", "abort-signal-cancellation-contract-missing"),
    runtime_rule!("abort-signal-lifecycle-contract" => "cancellation-liveness-boundary", "abort-signal-lifecycle-contract-missing"),
    runtime_rule!("abort-controller-signal-lifecycle-contract" => "cancellation-liveness-boundary", "abort-controller-signal-lifecycle-contract-missing"),
    RuntimeBoundaryRule {
        evidence: "promise-factory-settled-value-contract",
        obligation: (
            "success-error-result-channel",
            "promise-factory-settled-value-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-non-construct-call-boundary-contract",
        obligation: (
            "scheduling-boundary",
            "promise-non-construct-call-boundary-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "promise-like-receiver-proof",
        obligation: (
            "ambiguous-selector-boundary",
            "promise-like-receiver-proof-missing",
        ),
    },
    runtime_rule!("scheduler-wait-timing-contract" => "scheduling-boundary", "scheduler-wait-timing-contract-missing"),
    runtime_rule!("scheduler-wait-cancellation-liveness-contract" => "cancellation-liveness-boundary", "scheduler-wait-cancellation-liveness-contract-missing"),
    runtime_rule!("scheduler-yield-microtask-order-contract" => "scheduling-boundary", "scheduler-yield-microtask-order-contract-missing"),
    runtime_rule!("timer-scheduling-contract" => "scheduling-boundary", "timer-scheduling-contract-missing"),
    runtime_rule!("timer-cancellation-liveness-contract" => "cancellation-liveness-boundary", "timer-cancellation-liveness-contract-missing"),
    runtime_rule!("interval-async-iteration-lifecycle-contract" => "lifecycle-materialization-boundary", "interval-async-iteration-lifecycle-contract-missing"),
    runtime_rule!("interval-cancellation-liveness-contract" => "cancellation-liveness-boundary", "interval-cancellation-liveness-contract-missing"),
    runtime_rule!("task-spawn-scheduling-contract" => "scheduling-boundary", "task-spawn-scheduling-contract-missing"),
    runtime_rule!("task-yield-scheduling-contract" => "scheduling-boundary", "task-yield-scheduling-contract-missing"),
    runtime_rule!("task-handle-lifecycle-contract" => "lifecycle-materialization-boundary", "task-handle-lifecycle-contract-missing"),
    runtime_rule!("task-cancellation-liveness-contract" => "cancellation-liveness-boundary", "task-cancellation-liveness-contract-missing"),
    runtime_rule!("async-aggregate-all-completion-contract" => "success-error-result-channel", "async-aggregate-all-completion-contract-missing"),
    runtime_rule!("async-aggregate-first-completion-contract" => "cancellation-liveness-boundary", "async-aggregate-first-completion-contract-missing"),
    runtime_rule!("async-aggregate-completion-contract" => "success-error-result-channel", "async-aggregate-completion-contract-missing"),
    runtime_rule!("async-aggregate-result-channel-contract" => "success-error-result-channel", "async-aggregate-result-channel-contract-missing"),
    runtime_rule!("async-aggregate-cancellation-liveness-contract" => "cancellation-liveness-boundary", "async-aggregate-cancellation-liveness-contract-missing"),
    runtime_rule!("future-drive-scheduling-contract" => "scheduling-boundary", "future-drive-scheduling-contract-missing"),
    runtime_rule!("future-settled-value-channel-contract" => "success-error-result-channel", "future-settled-value-channel-contract-missing"),
    runtime_rule!("future-fulfillment-continuation-contract" => "success-error-result-channel", "future-fulfillment-continuation-contract-missing"),
    runtime_rule!("future-settlement-continuation-contract" => "success-error-result-channel", "future-settlement-continuation-contract-missing"),
    runtime_rule!("future-exception-continuation-contract" => "exception-channel", "future-exception-continuation-contract-missing"),
    runtime_rule!("future-callback-demand-effect-contract" => "callback-demand-effect", "future-callback-demand-effect-contract-missing"),
    runtime_rule!("channel-select-readiness-contract" => "channel-boundary", "channel-select-readiness-contract-missing"),
    runtime_rule!("channel-select-case-selection-contract" => "channel-boundary", "channel-select-case-selection-contract-missing"),
    runtime_rule!("channel-select-default-liveness-contract" => "channel-boundary", "channel-select-default-liveness-contract-missing"),
    runtime_rule!("channel-send-synchronization-contract" => "channel-boundary", "channel-send-synchronization-contract-missing"),
    runtime_rule!("channel-receive-status-contract" => "channel-boundary", "channel-receive-status-contract-missing"),
    runtime_rule!("channel-receive-value-channel-contract" => "channel-boundary", "channel-receive-value-channel-contract-missing"),
    RuntimeBoundaryRule {
        evidence: "channel-send-receive-protocol-contract",
        obligation: (
            "channel-boundary",
            "channel-send-receive-protocol-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "channel-select-protocol-contract",
        obligation: (
            "channel-boundary",
            "channel-select-protocol-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "channel-protocol-contract",
        obligation: ("channel-boundary", "channel-protocol-contract-missing"),
    },
    RuntimeBoundaryRule {
        evidence: "exception-channel-contract",
        obligation: ("exception-channel", "exception-channel-contract-missing"),
    },
    RuntimeBoundaryRule {
        evidence: "generator-yield-lifecycle-contract",
        obligation: (
            "lifecycle-materialization-boundary",
            "generator-yield-lifecycle-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "generator-yield-protocol-contract",
        obligation: (
            "lifecycle-materialization-boundary",
            "generator-yield-protocol-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "ruby-yield-callback-demand-effect-contract",
        obligation: (
            "callback-demand-effect",
            "ruby-yield-callback-demand-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "goroutine-scheduling-contract",
        obligation: (
            "scheduling-boundary",
            "goroutine-scheduling-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "defer-lifecycle-ordering-contract",
        obligation: (
            "lifecycle-materialization-boundary",
            "defer-lifecycle-ordering-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "defer-callback-effect-contract",
        obligation: (
            "callback-demand-effect",
            "defer-callback-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "concurrency-scheduling-contract",
        obligation: (
            "scheduling-boundary",
            "concurrency-scheduling-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "goroutine-callback-effect-contract",
        obligation: (
            "callback-demand-effect",
            "goroutine-callback-effect-contract-missing",
        ),
    },
    RuntimeBoundaryRule {
        evidence: "runtime-call-shape-contract",
        obligation: (
            "source-protocol-boundary",
            "runtime-call-shape-contract-missing",
        ),
    },
];

fn runtime_boundary_obligation(missing_evidence: &[&'static str]) -> Obligation {
    RUNTIME_BOUNDARY_OBLIGATIONS
        .iter()
        .find(|rule| missing_evidence.contains(&rule.evidence))
        .map(|rule| rule.obligation)
        .unwrap_or(DEFAULT_RUNTIME_BOUNDARY_OBLIGATION)
}

fn hof_demand_effect_obligation_subreason(missing_evidence: &[&'static str]) -> &'static str {
    if missing_evidence.contains(&"hof-callback-runtime-boundary-proof") {
        return "callback-runtime-boundary-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-assignment-effect-proof") {
        return "callback-assignment-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-direct-function-call-effect-proof") {
        return "callback-direct-function-call-effect-contract-missing";
    }
    if missing_evidence.contains(&"hof-callback-direct-method-call-effect-proof") {
        return "callback-direct-method-call-effect-contract-missing";
    }
    if missing_evidence.contains(&"hof-callback-imported-function-call-effect-proof") {
        return "callback-imported-function-call-effect-contract-missing";
    }
    if missing_evidence.contains(&"hof-callback-imported-member-call-effect-proof") {
        return "callback-imported-member-call-effect-contract-missing";
    }
    if missing_evidence.contains(&"hof-callback-dynamic-dispatch-call-effect-proof") {
        return "callback-dynamic-dispatch-call-effect-contract-missing";
    }
    if missing_evidence.contains(&"hof-callback-builtin-call-effect-proof") {
        return "callback-builtin-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-rust-macro-call-effect-proof") {
        return "callback-rust-macro-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-scoped-path-call-effect-proof") {
        return "callback-scoped-path-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-imported-binding-call-effect-proof") {
        return "callback-imported-binding-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-qualified-global-call-effect-proof") {
        return "callback-qualified-global-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-unshadowed-global-call-effect-proof") {
        return "callback-unshadowed-global-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-member-call-effect-proof") {
        return "callback-member-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-local-or-parameter-call-effect-proof") {
        return "callback-local-or-parameter-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-rejected-call-target-effect-proof") {
        return "callback-rejected-call-target-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-unknown-call-effect-proof") {
        return "callback-unknown-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-call-effect-proof") {
        return "callback-call-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-effect-proof") {
        return "callback-effect-proof-missing";
    }
    if missing_evidence.contains(&"hof-callback-identity-proof")
        || missing_evidence.contains(&"hof-callback-arity-shape-proof")
    {
        return "callback-identity-or-shape-proof-missing";
    }
    if missing_evidence.contains(&"hof-reduce-callback-demand-effect-profile") {
        return "reduction-callback-demand-effect-profile-missing";
    }
    if missing_evidence.contains(&"hof-filter-map-callback-demand-effect-profile") {
        return "optional-callback-demand-effect-profile-missing";
    }
    if missing_evidence.contains(&"hof-flat-map-callback-demand-effect-profile") {
        return "flattening-callback-demand-effect-profile-missing";
    }
    if missing_evidence.contains(&"hof-filter-callback-demand-effect-profile")
        || missing_evidence.contains(&"hof-reject-callback-demand-effect-profile")
    {
        return "predicate-callback-demand-effect-profile-missing";
    }
    if missing_evidence.contains(&"hof-map-callback-demand-effect-profile") {
        return "mapping-callback-demand-effect-profile-missing";
    }
    if missing_evidence.contains(&"hof-source-or-library-api-occurrence-proof") {
        return "hof-source-or-library-api-occurrence-proof-missing";
    }
    "hof-demand-effect-profile-missing"
}

#[cfg(test)]
mod tests;
