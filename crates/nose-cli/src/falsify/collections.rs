use super::FalsifyTarget;
use nose_detect::OracleInputProjection;
use nose_il::{Il, NodeId, NodeKind};

pub(crate) fn collection_input_projections(
    il: &Il,
    interner: &nose_il::Interner,
    root: NodeId,
    projections: &[OracleInputProjection],
) -> Vec<OracleInputProjection> {
    let params = il
        .children(root)
        .iter()
        .filter(|&&id| il.kind(id) == NodeKind::Param)
        .collect::<Vec<_>>();
    if params.len() != projections.len() {
        return projections.to_vec();
    }
    params
        .iter()
        .zip(projections)
        .map(|(&param, &projection)| {
            if projection == OracleInputProjection::Declared {
                nose_semantics::array_element_domain_for_param(il, *param)
                    .map(OracleInputProjection::ScalarArray)
                    .or_else(|| {
                        nose_semantics::keyed_membership_projection(il, interner, root, *param)
                            .map(OracleInputProjection::KeyedMembership)
                    })
                    .unwrap_or(projection)
            } else {
                projection
            }
        })
        .collect()
}

pub(super) fn valid_collection_projections(
    target: FalsifyTarget<'_>,
    interner: &nose_il::Interner,
) -> bool {
    let params = target
        .il
        .children(target.root)
        .iter()
        .filter(|&&id| target.il.kind(id) == NodeKind::Param);
    params
        .zip(target.projections)
        .all(|(&param, projection)| match projection {
            OracleInputProjection::ScalarArray(element) => {
                nose_semantics::array_element_domain_for_param(target.il, param) == Some(*element)
            }
            OracleInputProjection::KeyedMembership(key) => {
                nose_semantics::keyed_membership_projection(target.il, interner, target.root, param)
                    == Some(*key)
            }
            _ => true,
        })
}
