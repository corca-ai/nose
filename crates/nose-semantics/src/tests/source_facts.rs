use super::*;

#[test]
fn source_fact_contracts_are_span_keyed_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let call = b.add(NodeKind::Call, Payload::None, sp(7), &[]);
    let regex = b.add(NodeKind::Lit, Payload::LitStr(42), sp(8), &[]);
    let await_boundary = b.add(NodeKind::Raw, Payload::None, sp(9), &[]);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(7),
        &[call, regex, await_boundary],
    );
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(7)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(8)),
        EvidenceKind::Source(SourceFactKind::Literal(SourceLiteralKind::Regex)),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        2,
        EvidenceAnchor::source_span(sp(9)),
        EvidenceKind::Source(SourceFactKind::Protocol(SourceProtocolKind::Await)),
        EvidenceStatus::Asserted,
    ));

    assert!(construct_syntax_proof(&il, call));
    assert!(regex_literal_proof(&il, regex));
    assert_eq!(
        source_protocol_at_node(&il, await_boundary),
        Some(SourceProtocolKind::Await)
    );
    assert!(!construct_syntax_proof(&il, regex));
    assert_eq!(
        source_fact_contract(SourceFactKind::Call(SourceCallKind::Construct)).channel,
        ChannelEligibility::ExactProven
    );
}

#[test]
fn source_fact_evidence_conflicts_fail_closed() {
    let mut b = IlBuilder::new(FileId(0));
    let op = b.add(NodeKind::BinOp, Payload::Op(Op::Eq), sp(9), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(9), &[op]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(9)),
        EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::StrictEquality)),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        source_operator_at_node(&il, op),
        Some(SourceOperatorKind::StrictEquality)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(9)),
        EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::LooseEquality)),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(source_operator_at_node(&il, op), None);
}

#[test]
fn source_range_and_pattern_facts_are_span_keyed_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let range = b.add(NodeKind::Seq, Payload::None, sp(12), &[]);
    let pattern = b.add(NodeKind::Raw, Payload::None, sp(13), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(12), &[range, pattern]);
    let mut il = finish_il(b, root, Lang::Rust);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(12)),
        EvidenceKind::Source(SourceFactKind::Range(
            SourceRangeKind::RustHalfOpenRangeExpression,
        )),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(13)),
        EvidenceKind::Source(SourceFactKind::Pattern(
            SourcePatternKind::RustTupleStructSingleWildcardPattern,
        )),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        source_range_at_node(&il, range),
        Some(SourceRangeKind::RustHalfOpenRangeExpression)
    );
    assert_eq!(
        source_pattern_at_node(&il, pattern),
        Some(SourcePatternKind::RustTupleStructSingleWildcardPattern)
    );
    assert!(source_fact_at_node(
        &il,
        range,
        SourceFactKind::Range(SourceRangeKind::RustHalfOpenRangeExpression)
    ));
    assert!(!source_fact_at_node(
        &il,
        pattern,
        SourceFactKind::Range(SourceRangeKind::RustHalfOpenRangeExpression)
    ));
}

#[test]
fn source_range_fact_conflicts_fail_closed() {
    let mut b = IlBuilder::new(FileId(0));
    let range = b.add(NodeKind::Seq, Payload::None, sp(14), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(14), &[range]);
    let mut il = finish_il(b, root, Lang::Rust);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(sp(14)),
        EvidenceKind::Source(SourceFactKind::Range(
            SourceRangeKind::RustHalfOpenRangeExpression,
        )),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        source_range_at_node(&il, range),
        Some(SourceRangeKind::RustHalfOpenRangeExpression)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(14)),
        EvidenceKind::Source(SourceFactKind::Range(
            SourceRangeKind::RustInclusiveRangeExpression,
        )),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(source_range_at_node(&il, range), None);
}

#[test]
fn source_fact_evidence_requires_live_dependencies() {
    let mut b = IlBuilder::new(FileId(0));
    let call = b.add(NodeKind::Call, Payload::None, sp(10), &[]);
    let cast = b.add(NodeKind::Call, Payload::None, sp(11), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(10), &[call, cast]);
    let mut il = finish_il(b, root, Lang::Rust);
    il.evidence.push(evidence_with_dependencies(
        0,
        EvidenceAnchor::source_span(sp(10)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::MacroInvocation)),
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    ));

    assert_eq!(source_call_at_node(&il, call), None);
    assert!(!source_fact_at_node(
        &il,
        call,
        SourceFactKind::Call(SourceCallKind::MacroInvocation),
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::source_span(sp(11)),
        EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
        EvidenceStatus::Asserted,
        vec![EvidenceId(100)],
    ));
    assert_eq!(source_cast_at_node(&il, cast), None);
    assert!(!source_fact_at_node(
        &il,
        cast,
        SourceFactKind::Cast(SourceCastKind::CUnsigned32),
    ));
}
