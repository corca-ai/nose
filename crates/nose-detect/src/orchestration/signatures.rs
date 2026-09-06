//! Finish deferred signatures once per equal feature multiset in this analysis.
use crate::{minhash, UnitFeat};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

#[cfg(test)]
mod tests;

#[derive(Default)]
struct Inputs<'a> {
    index: FxHashMap<&'a [u64], usize>,
    features: Vec<&'a [u64]>,
    uses: Vec<usize>,
}

impl<'a> Inputs<'a> {
    fn intern(&mut self, features: &'a [u64]) -> usize {
        let i = *self.index.entry(features).or_insert_with(|| {
            let i = self.features.len();
            self.features.push(features);
            self.uses.push(0);
            i
        });
        self.uses[i] += 1;
        i
    }
}

pub(super) fn finish(units: &mut [UnitFeat], shape_features: bool, seeds: &[u64]) {
    let mut inputs = Inputs::default();
    let assignments = units
        .iter()
        .map(|unit| {
            let shape = shape_features.then(|| inputs.intern(&unit.shapes));
            let value = if unit.value.is_empty() {
                shape
            } else {
                Some(inputs.intern(&unit.value))
            };
            if unit.value.is_empty() {
                if let Some(i) = value {
                    inputs.uses[i] += 1;
                }
            }
            (shape, value)
        })
        .collect::<Vec<_>>();
    let mut signatures = inputs
        .features
        .par_iter()
        .map(|features| {
            let mut distinct = features.to_vec();
            distinct.dedup();
            minhash::sign(&distinct, seeds)
        })
        .collect::<Vec<_>>();
    let mut uses = inputs.uses;
    drop(inputs.index);
    drop(inputs.features);
    for (unit, (shape, value)) in units.iter_mut().zip(assignments) {
        unit.shape_minhash = take_signature(&mut signatures, &mut uses, shape);
        unit.minhash = take_signature(&mut signatures, &mut uses, value);
    }
}

fn take_signature(signatures: &mut [Vec<u64>], uses: &mut [usize], i: Option<usize>) -> Vec<u64> {
    let Some(i) = i else {
        return Vec::new();
    };
    uses[i] -= 1;
    if uses[i] == 0 {
        std::mem::take(&mut signatures[i])
    } else {
        signatures[i].clone()
    }
}
