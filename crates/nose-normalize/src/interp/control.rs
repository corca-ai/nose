use super::*;

impl Interp<'_> {
    pub(super) fn tick(&mut self) -> R<()> {
        self.steps += 1;
        if self.steps > STEP_BUDGET {
            Err(Unsupported::budget("budget.interpreter-steps"))
        } else {
            Ok(())
        }
    }

    /// Truthiness of an If/ternary condition. A concrete value decides as usual.
    /// A symbolic condition bails in strict mode; under #244 exploration it takes
    /// the prescribed arm (or assumes `true` at a new site, depth-first) and
    /// RECORDS the assumption as a `Sym` effect marker — so the decision is
    /// conditioned, never guessed, and any cross-unit disagreement involving an
    /// explored path stays in the advisory lane (the marker keeps the behavior
    /// symbolic). Loop conditions deliberately stay strict: an assumption per
    /// iteration is an unbounded chain, not a bounded fork.
    pub(super) fn cond_truthy(&mut self, v: &Value) -> R<bool> {
        if let Some(t) = self.value_truthy(v) {
            return Ok(t);
        }
        let Value::Sym(h) = v else {
            return Err(Unsupported::value("value.condition-truthiness"));
        };
        let h = *h;
        let Some(ex) = self.explore.as_mut() else {
            return Err(Unsupported::value("value.symbolic-condition"));
        };
        if ex.taken.len() >= MAX_SYM_BRANCH_SITES {
            ex.cap_hit = true;
            return Err(Unsupported::budget("budget.symbolic-branch-sites"));
        }
        let taken = ex.prescribed.get(ex.taken.len()).copied().unwrap_or(true);
        ex.taken.push(taken);
        self.effects
            .push(Value::Sym(sym_id(SYM_ASSUME, &[h, u64::from(taken)])));
        Ok(taken)
    }

    /// Concrete truthiness with the source language's Number edge cases. JavaScript treats
    /// NaN as falsy and every array object as truthy; Python deliberately does neither, so
    /// these cases cannot live in the shared helper.
    pub(super) fn value_truthy(&self, v: &Value) -> Option<bool> {
        if self.bitwise_result_is_int32() {
            match v {
                Value::Float(value) if value.0.is_nan() => return Some(false),
                Value::List(_) => return Some(true),
                _ => {}
            }
        }
        truthy(v)
    }
}
