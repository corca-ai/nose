//! Free-function builtin contract rows.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinArgContract {
    First,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeFunctionBuiltinContract {
    pub name: &'static str,
    pub builtin: Builtin,
    pub args: BuiltinArgContract,
    pub requires_unshadowed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FreeFunctionHofContract {
    pub name: &'static str,
    pub kind: HoFKind,
    pub source_arg: usize,
    pub callback_arg: usize,
    pub requires_unshadowed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FreeFunctionBuiltinArity {
    Exact(usize),
    AtLeast(usize),
    OneOf(&'static [usize]),
}

impl FreeFunctionBuiltinArity {
    fn accepts(self, arg_count: usize) -> bool {
        match self {
            FreeFunctionBuiltinArity::Exact(expected) => arg_count == expected,
            FreeFunctionBuiltinArity::AtLeast(minimum) => arg_count >= minimum,
            FreeFunctionBuiltinArity::OneOf(expected) => expected.contains(&arg_count),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FreeFunctionBuiltinRow {
    lang: Lang,
    name: &'static str,
    builtin: Builtin,
    args: BuiltinArgContract,
    arity: FreeFunctionBuiltinArity,
    requires_unshadowed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FreeFunctionHofRow {
    lang: Lang,
    name: &'static str,
    kind: HoFKind,
    arity: FreeFunctionBuiltinArity,
    source_arg: usize,
    callback_arg: usize,
    requires_unshadowed: bool,
}

const ONE_OR_TWO_ARGS: &[usize] = &[1, 2];
const ONE_TO_THREE_ARGS: &[usize] = &[1, 2, 3];
const PY: Lang = Lang::Python;
const GO: Lang = Lang::Go;
const SW: Lang = Lang::Swift;
const FIRST_ARG: BuiltinArgContract = BuiltinArgContract::First;
const ALL_ARGS: BuiltinArgContract = BuiltinArgContract::All;
const ARITY_ANY: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::AtLeast(0);
const ARITY_ONE: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::Exact(1);
const ARITY_TWO: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::Exact(2);
const ARITY_AT_LEAST_TWO: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::AtLeast(2);
const ARITY_ONE_OR_TWO: FreeFunctionBuiltinArity = FreeFunctionBuiltinArity::OneOf(ONE_OR_TWO_ARGS);
const ARITY_ONE_TO_THREE: FreeFunctionBuiltinArity =
    FreeFunctionBuiltinArity::OneOf(ONE_TO_THREE_ARGS);

const fn free_function_builtin_row(
    lang: Lang,
    name: &'static str,
    builtin: Builtin,
    args: BuiltinArgContract,
    arity: FreeFunctionBuiltinArity,
) -> FreeFunctionBuiltinRow {
    FreeFunctionBuiltinRow {
        lang,
        name,
        builtin,
        args,
        arity,
        requires_unshadowed: true,
    }
}

const fn free_function_hof_row(
    lang: Lang,
    name: &'static str,
    kind: HoFKind,
    arity: FreeFunctionBuiltinArity,
    source_arg: usize,
    callback_arg: usize,
) -> FreeFunctionHofRow {
    FreeFunctionHofRow {
        lang,
        name,
        kind,
        arity,
        source_arg,
        callback_arg,
        requires_unshadowed: true,
    }
}

const FREE_FUNCTION_BUILTINS: &[FreeFunctionBuiltinRow] = &[
    free_function_builtin_row(PY, "len", Builtin::Len, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(GO, "len", Builtin::Len, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(GO, "append", Builtin::Append, ALL_ARGS, ARITY_AT_LEAST_TWO),
    free_function_builtin_row(PY, "print", Builtin::Print, ALL_ARGS, ARITY_ANY),
    free_function_builtin_row(PY, "range", Builtin::Range, ALL_ARGS, ARITY_ONE_TO_THREE),
    free_function_builtin_row(PY, "sum", Builtin::Sum, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "min", Builtin::Min, ALL_ARGS, ARITY_ONE_OR_TWO),
    free_function_builtin_row(PY, "max", Builtin::Max, ALL_ARGS, ARITY_ONE_OR_TWO),
    free_function_builtin_row(PY, "abs", Builtin::Abs, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(SW, "min", Builtin::Min, ALL_ARGS, ARITY_AT_LEAST_TWO),
    free_function_builtin_row(SW, "max", Builtin::Max, ALL_ARGS, ARITY_AT_LEAST_TWO),
    free_function_builtin_row(SW, "abs", Builtin::Abs, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "zip", Builtin::Zip, ALL_ARGS, ARITY_TWO),
    free_function_builtin_row(PY, "enumerate", Builtin::Enumerate, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "any", Builtin::Any, FIRST_ARG, ARITY_ONE),
    free_function_builtin_row(PY, "all", Builtin::All, FIRST_ARG, ARITY_ONE),
];

const FREE_FUNCTION_HOFS: &[FreeFunctionHofRow] = &[
    free_function_hof_row(PY, "map", HoFKind::Map, ARITY_TWO, 1, 0),
    free_function_hof_row(PY, "filter", HoFKind::Filter, ARITY_TWO, 1, 0),
];

fn free_function_builtin_contract_from_row(
    row: &FreeFunctionBuiltinRow,
) -> FreeFunctionBuiltinContract {
    FreeFunctionBuiltinContract {
        name: row.name,
        builtin: row.builtin,
        args: row.args,
        requires_unshadowed: row.requires_unshadowed,
    }
}

fn free_function_hof_contract_from_row(row: &FreeFunctionHofRow) -> FreeFunctionHofContract {
    FreeFunctionHofContract {
        name: row.name,
        kind: row.kind,
        source_arg: row.source_arg,
        callback_arg: row.callback_arg,
        requires_unshadowed: row.requires_unshadowed,
    }
}

pub fn free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<FreeFunctionBuiltinContract> {
    FREE_FUNCTION_BUILTINS
        .iter()
        .find(|row| row.lang == lang && row.name == name && row.arity.accepts(arg_count))
        .map(free_function_builtin_contract_from_row)
}

pub fn free_function_hof_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<FreeFunctionHofContract> {
    FREE_FUNCTION_HOFS
        .iter()
        .find(|row| row.lang == lang && row.name == name && row.arity.accepts(arg_count))
        .map(free_function_hof_contract_from_row)
}
