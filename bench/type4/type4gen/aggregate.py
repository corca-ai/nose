from __future__ import annotations

import math

from .model import (
    OPERATIONS,
    PAIR_PROPERTY_INPUTS,
    PROPERTY_INPUTS,
    Operation,
    Surface,
    Variant,
    js_script_wrap,
    js_start_line,
    snake_to_camel,
    snake_to_pascal,
)

def render_predicate(predicate: str, var: str) -> str:
    if predicate == "gt0":
        return f"{var} > 0"
    if predicate == "ge0":
        return f"{var} >= 0"
    if predicate == "lt0":
        return f"{var} < 0"
    if predicate == "le0":
        return f"{var} <= 0"
    if predicate == "eq0":
        return f"{var} == 0"
    if predicate == "ne0":
        return f"{var} != 0"
    if predicate == "even":
        return f"{var} % 2 == 0"
    if predicate == "odd":
        return f"{var} % 2 != 0"
    if predicate == "lt3":
        return f"{var} < 3"
    if predicate == "le3":
        return f"{var} <= 3"
    if predicate == "true":
        return "1 == 1"
    raise ValueError(f"unknown predicate: {predicate}")


def render_contribution(contribution: str, var: str, other: str | None = None) -> str:
    if contribution == "identity":
        return var
    if contribution == "square":
        return f"{var} * {var}"
    if contribution == "pair_product" and other is not None:
        return f"{var} * {other}"
    if contribution == "pair_sum" and other is not None:
        return f"{var} + {other}"
    raise ValueError(f"unknown contribution: {contribution}")


def negate(expr: str, language: str) -> str:
    if language == "python":
        return f"not ({expr})"
    return f"!({expr})"


def eval_predicate(predicate: str, x: int) -> bool:
    if predicate == "gt0":
        return x > 0
    if predicate == "ge0":
        return x >= 0
    if predicate == "lt0":
        return x < 0
    if predicate == "le0":
        return x <= 0
    if predicate == "eq0":
        return x == 0
    if predicate == "ne0":
        return x != 0
    if predicate == "even":
        return x % 2 == 0
    if predicate == "odd":
        return x % 2 != 0
    if predicate == "lt3":
        return x < 3
    if predicate == "le3":
        return x <= 3
    if predicate == "true":
        return True
    raise ValueError(f"unknown predicate: {predicate}")


def eval_contribution(contribution: str, x: int, y: int | None = None) -> int:
    if contribution == "identity":
        return x
    if contribution == "square":
        return x * x
    if contribution == "pair_product" and y is not None:
        return x * y
    if contribution == "pair_sum" and y is not None:
        return x + y
    raise ValueError(f"unknown contribution: {contribution}")


def effective_operation(operation: Operation, negative: bool) -> tuple[str, int]:
    predicate = operation.positive_predicate
    init = operation.positive_init
    if negative:
        if operation.negative_predicate is not None:
            predicate = operation.negative_predicate
        if operation.negative_init is not None:
            init = operation.negative_init
    return predicate, init


def effective_components(operation: Operation, negative: bool) -> tuple[str, str, int, str]:
    kind = operation.kind
    predicate, init = effective_operation(operation, negative)
    contribution = operation.positive_contribution
    if negative and operation.negative_kind is not None:
        kind = operation.negative_kind
    if negative and operation.negative_contribution is not None:
        contribution = operation.negative_contribution
    return kind, predicate, init, contribution


def property_inputs(operation_key: str) -> list[dict]:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return PAIR_PROPERTY_INPUTS
    return [{"xs": xs} for xs in PROPERTY_INPUTS]


def spec_output(
    operation_key: str,
    inputs: dict | list[int],
    negative: bool = False,
    start: int = 0,
    step: int = 1,
) -> int | bool:
    op = OPERATIONS[operation_key]
    kind, predicate, init, contribution = effective_components(op, negative)
    if isinstance(inputs, list):
        inputs = {"xs": inputs}
    if op.arity == 2:
        pairs = list(zip(inputs["a"], inputs["b"]))[start::step]
        selected_values = [
            eval_contribution(contribution, x, y)
            for x, y in pairs
            if eval_predicate(predicate, x)
        ]
        if kind == "sum":
            return init + sum(selected_values)
        raise ValueError(f"unsupported arity-2 operation kind: {kind}")
    view = inputs["xs"][start::step]
    selected = [x for x in view if eval_predicate(predicate, x)]
    selected_values = [eval_contribution(contribution, x) for x in selected]
    if kind == "sum":
        return init + sum(selected_values)
    if kind == "sum_abs":
        return init + sum(abs(x) for x in selected)
    if kind == "count":
        return len(selected)
    if kind == "any":
        return any(eval_predicate(predicate, x) for x in view)
    if kind == "all":
        return all(eval_predicate(predicate, x) for x in view)
    if kind == "product":
        return math.prod(selected_values, start=init)
    if kind == "max":
        return max([init, *selected])
    if kind == "min":
        return min([init, *selected])
    raise ValueError(f"unknown operation kind: {kind}")


def counterexample(operation_key: str) -> dict:
    for inputs in property_inputs(operation_key):
        left = spec_output(operation_key, inputs, negative=False)
        right = spec_output(operation_key, inputs, negative=True)
        if left != right:
            return {"input": inputs, "left_output": left, "right_output": right}
    raise ValueError(f"no counterexample in property inputs for {operation_key}")


def evidence_positive(operation_key: str) -> dict:
    inputs = property_inputs(operation_key)
    return {
        "level": "E1",
        "kind": "same-spec-template+spec-interpreter",
        "property_inputs": inputs,
        "outputs": [spec_output(operation_key, inp, negative=False) for inp in inputs],
    }


def evidence_negative(operation_key: str) -> dict:
    return {
        "level": "E2",
        "kind": "counterexample",
        "counterexample": counterexample(operation_key),
    }


def contract_negative_window(representation: str) -> tuple[int, int]:
    if representation == "c_start_one":
        return 1, 1
    if representation == "c_stride_two":
        return 0, 2
    raise ValueError(f"unknown C contract negative representation: {representation}")


def contract_counterexample(operation_key: str, representation: str) -> dict:
    start, step = contract_negative_window(representation)
    for inputs in property_inputs(operation_key):
        left = spec_output(operation_key, inputs)
        right = spec_output(operation_key, inputs, start=start, step=step)
        if left != right:
            return {"input": inputs, "left_output": left, "right_output": right}
    raise ValueError(f"no C contract counterexample for {operation_key} {representation}")


def evidence_contract_negative(operation_key: str, representation: str) -> dict:
    return {
        "level": "E2",
        "kind": "counterexample",
        "counterexample": contract_counterexample(operation_key, representation),
    }


def python_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return python_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for x in xs:" if representation == "loop" else "for i in range(len(xs)):"
            loop_bind = "" if representation == "loop" else "\n        x = xs[i]"
            src = f"""def {operation_key}(xs):
    total = {init}
    {loop_head}{loop_bind}
        if x < 0:
            total += -x
        else:
            total += x
    return total
"""
        else:
            prefix = "" if init == 0 else f"{init} + "
            src = f"""def {operation_key}(xs):
    return {prefix}sum(abs(x) for x in xs)
"""
        return Variant(representation, src, operation_key)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for x in xs:" if representation == "loop" else "for i in range(len(xs)):"
        loop_bind = "" if representation == "loop" else "\n        x = xs[i]"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {
                "sum": f"total += {term}",
                "count": "count += 1",
                "product": f"product *= {term}",
            }[kind]
            src = f"""def {operation_key}(xs):
    {var} = {init}
    {loop_head}{loop_bind}
        if {pred}:
            {update}
    return {var}
"""
        elif kind == "any":
            src = f"""def {operation_key}(xs):
    {loop_head}{loop_bind}
        if {pred}:
            return True
    return False
"""
        elif kind == "all":
            src = f"""def {operation_key}(xs):
    {loop_head}{loop_bind}
        if {negate(pred, "python")}:
            return False
    return True
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            src = f"""def {operation_key}(xs):
    best = {init}
    {loop_head}{loop_bind}
        if x {cmp} best:
            best = x
    return best
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        prefix = "" if init == 0 else f"{init} + "
        src = f"""def {operation_key}(xs):
    return {prefix}sum({term} for x in xs if {pred})
"""
    elif kind == "count":
        src = f"""def {operation_key}(xs):
    return sum(1 for x in xs if {pred})
"""
    elif kind == "any":
        src = f"""def {operation_key}(xs):
    return any({pred} for x in xs)
"""
    elif kind == "all":
        src = f"""def {operation_key}(xs):
    return all({pred} for x in xs)
"""
    elif kind == "product":
        src = f"""import math

def {operation_key}(xs):
    return math.prod(({term} for x in xs if {pred}), start={init})
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        src = f"""def {operation_key}(xs):
    return reduce(lambda best, x: x if x {cmp} best else best, xs, {init})
"""
    else:
        raise ValueError(kind)
    return Variant(representation, src, operation_key)


def js_variant(surface: Surface, operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return js_pair_variant(surface, operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for (const x of xs)" if representation == "loop" else "for (let i = 0; i < xs.length; i += 1)"
            loop_bind = "" if representation == "loop" else "\n    const x = xs[i];"
            body = f"""function {name}(xs) {{
  let total = {init};
  {loop_head} {{{loop_bind}
    if (x < 0) {{
      total += -x;
    }} else {{
      total += x;
    }}
  }}
  return total;
}}
"""
        else:
            body = f"""function {name}(xs) {{
  return xs.reduce((total, x) => total + (x < 0 ? -x : x), {init});
}}
"""
        return Variant(representation, js_script_wrap(surface, body), name, js_start_line(surface))
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for (const x of xs)" if representation == "loop" else "for (let i = 0; i < xs.length; i += 1)"
        loop_bind = "" if representation == "loop" else "\n    const x = xs[i];"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term};", "count": "count += 1;", "product": f"product *= {term};"}[
                kind
            ]
            body = f"""function {name}(xs) {{
  let {var} = {init};
  {loop_head} {{{loop_bind}
    if ({pred}) {{
      {update}
    }}
  }}
  return {var};
}}
"""
        elif kind == "any":
            body = f"""function {name}(xs) {{
  {loop_head} {{{loop_bind}
    if ({pred}) {{
      return true;
    }}
  }}
  return false;
}}
"""
        elif kind == "all":
            body = f"""function {name}(xs) {{
  {loop_head} {{{loop_bind}
    if ({negate(pred, "javascript")}) {{
      return false;
    }}
  }}
  return true;
}}
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            body = f"""function {name}(xs) {{
  let best = {init};
  {loop_head} {{{loop_bind}
    if (x {cmp} best) {{
      best = x;
    }}
  }}
  return best;
}}
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        body = f"""function {name}(xs) {{
  return xs.filter((x) => {pred}).reduce((total, x) => total + {term}, {init});
}}
"""
    elif kind == "count":
        body = f"""function {name}(xs) {{
  return xs.filter((x) => {pred}).length;
}}
"""
    elif kind == "any":
        body = f"""function {name}(xs) {{
  return xs.some((x) => {pred});
}}
"""
    elif kind == "all":
        body = f"""function {name}(xs) {{
  return xs.every((x) => {pred});
}}
"""
    elif kind == "product":
        body = f"""function {name}(xs) {{
  return xs.filter((x) => {pred}).reduce((product, x) => product * {term}, {init});
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""function {name}(xs) {{
  return xs.reduce((best, x) => x {cmp} best ? x : best, {init});
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, js_script_wrap(surface, body), name, js_start_line(surface))


def typescript_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return typescript_pair_variant(operation_key, representation, negative)
    v = js_variant(Surface("typescript", "typescript", "ts"), operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    kind, _, _, _ = effective_components(op, negative)
    ret = "boolean" if kind in {"any", "all"} else "number"
    src = v.source.replace(f"function {name}(xs)", f"function {name}(xs: number[]): {ret}")
    return Variant(representation, src, name)


def go_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return go_pair_variant(operation_key, representation, negative)
    name = snake_to_pascal(operation_key)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if representation == "loop":
        iter_head = "for _, x := range xs"
        index_line = ""
    elif representation in {"aggregate", "indexed_loop"}:
        iter_head = "for i := 0; i < len(xs); i++"
        index_line = "\n\t\tx := xs[i]"
    else:
        raise ValueError(f"unknown Go representation: {representation}")
    if kind == "sum_abs":
        body = f"""package main

func {name}(xs []int) int {{
	total := {init}
	{iter_head} {{{index_line}
		if x < 0 {{
			total += -x
		}} else {{
			total += x
		}}
	}}
	return total
}}
"""
        return Variant(representation, body, name)
    if kind in {"sum", "count", "product"}:
        var = {"sum": "total", "count": "count", "product": "product"}[kind]
        update = {"sum": f"total += {term}", "count": "count += 1", "product": f"product *= {term}"}[kind]
        body = f"""package main

func {name}(xs []int) int {{
	{var} := {init}
	{iter_head} {{{index_line}
		if {pred} {{
			{update}
		}}
	}}
	return {var}
}}
"""
    elif kind == "any":
        body = f"""package main

func {name}(xs []int) bool {{
	{iter_head} {{{index_line}
		if {pred} {{
			return true
		}}
	}}
	return false
}}
"""
    elif kind == "all":
        body = f"""package main

func {name}(xs []int) bool {{
	{iter_head} {{{index_line}
		if {negate(pred, "go")} {{
			return false
		}}
	}}
	return true
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""package main

func {name}(xs []int) int {{
	best := {init}
	{iter_head} {{{index_line}
		if x {cmp} best {{
			best = x
		}}
	}}
	return best
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, body, name)


def rust_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return rust_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred_val = render_predicate(predicate_key, "x")
    pred_ref = render_predicate(predicate_key, "*x")
    term = render_contribution(contribution, "x")
    ret = "bool" if kind in {"any", "all"} else "i32"
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for &x in xs" if representation == "loop" else "for i in 0..xs.len()"
            loop_bind = "" if representation == "loop" else "\n        let x = xs[i];"
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    let mut total = {init};
    {loop_head} {{{loop_bind}
        if x < 0 {{
            total += -x;
        }} else {{
            total += x;
        }}
    }}
    total
}}
"""
        else:
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().fold({init}, |total, x| total + if x < 0 {{ -x }} else {{ x }})
}}
"""
        return Variant(representation, src, operation_key)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for &x in xs" if representation == "loop" else "for i in 0..xs.len()"
        loop_bind = "" if representation == "loop" else "\n        let x = xs[i];"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term};", "count": "count += 1;", "product": f"product *= {term};"}[
                kind
            ]
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    let mut {var} = {init};
    {loop_head} {{{loop_bind}
        if {pred_val} {{
            {update}
        }}
    }}
    {var}
}}
"""
        elif kind == "any":
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    {loop_head} {{{loop_bind}
        if {pred_val} {{
            return true;
        }}
    }}
    false
}}
"""
        elif kind == "all":
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    {loop_head} {{{loop_bind}
        if {negate(pred_val, "rust")} {{
            return false;
        }}
    }}
    true
}}
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    let mut best = {init};
    {loop_head} {{{loop_bind}
        if x {cmp} best {{
            best = x;
        }}
    }}
    best
}}
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().filter(|x| {pred_ref}).fold({init}, |total, x| total + {term})
}}
"""
    elif kind == "count":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().filter(|x| {pred_ref}).count() as i32
}}
"""
    elif kind == "any":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().any(|x| {pred_val})
}}
"""
    elif kind == "all":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().all(|x| {pred_val})
}}
"""
    elif kind == "product":
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().filter(|x| {pred_ref}).fold({init}, |product, x| product * {term})
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        src = f"""pub fn {operation_key}(xs: &[i32]) -> {ret} {{
    xs.iter().copied().fold({init}, |best, x| if x {cmp} best {{ x }} else {{ best }})
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, src, operation_key)


def java_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return java_pair_variant(operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    class_name = f"Case{snake_to_pascal(operation_key)}{representation.title()}"
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    ret = "boolean" if kind in {"any", "all"} else "int"
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "for (int x : xs)" if representation == "loop" else "for (int i = 0; i < xs.length; i++)"
            loop_bind = "" if representation == "loop" else "\n      int x = xs[i];"
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    int total = {init};
    {loop_head} {{{loop_bind}
      if (x < 0) {{
        total += -x;
      }} else {{
        total += x;
      }}
    }}
    return total;
  }}
}}
"""
        else:
            body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).reduce({init}, (total, x) -> total + (x < 0 ? -x : x));
  }}
}}
"""
        return Variant(representation, body, name)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "for (int x : xs)" if representation == "loop" else "for (int i = 0; i < xs.length; i++)"
        loop_bind = "" if representation == "loop" else "\n      int x = xs[i];"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term};", "count": "count += 1;", "product": f"product *= {term};"}[
                kind
            ]
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    int {var} = {init};
    {loop_head} {{{loop_bind}
      if ({pred}) {{
        {update}
      }}
    }}
    return {var};
  }}
}}
"""
        elif kind == "any":
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    {loop_head} {{{loop_bind}
      if ({pred}) {{
        return true;
      }}
    }}
    return false;
  }}
}}
"""
        elif kind == "all":
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    {loop_head} {{{loop_bind}
      if ({negate(pred, "java")}) {{
        return false;
      }}
    }}
    return true;
  }}
}}
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            body = f"""class {class_name} {{
  static {ret} {name}(int[] xs) {{
    int best = {init};
    {loop_head} {{{loop_bind}
      if (x {cmp} best) {{
        best = x;
      }}
    }}
    return best;
  }}
}}
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).filter(x -> {pred}).reduce({init}, (total, x) -> total + {term});
  }}
}}
"""
    elif kind == "count":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return (int) Arrays.stream(xs).filter(x -> {pred}).count();
  }}
}}
"""
    elif kind == "any":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).anyMatch(x -> {pred});
  }}
}}
"""
    elif kind == "all":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).allMatch(x -> {pred});
  }}
}}
"""
    elif kind == "product":
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).filter(x -> {pred}).reduce({init}, (product, x) -> product * {term});
  }}
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""import java.util.Arrays;

class {class_name} {{
  static {ret} {name}(int[] xs) {{
    return Arrays.stream(xs).reduce({init}, (best, x) -> x {cmp} best ? x : best);
  }}
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, body, name)


def c_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return c_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "xs[i]")
    term = render_contribution(contribution, "xs[i]")
    if representation == "loop":
        iter_head = "for (int i = 0; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_start_one":
        iter_head = "for (int i = 1; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_stride_two":
        iter_head = "for (int i = 0; i < n; i += 2)"
        inc = ""
        prefix = ""
    elif representation in {"aggregate", "indexed_loop"}:
        iter_head = "while (i < n)"
        inc = "\n    i++;"
        prefix = "\n  int i = 0;"
    else:
        raise ValueError(f"unknown C representation: {representation}")
    if kind == "sum_abs":
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  int total = {init};
  {iter_head} {{
    if (xs[i] < 0) {{
      total += -xs[i];
    }} else {{
      total += xs[i];
    }}{inc}
  }}
  return total;
}}
"""
        return Variant(representation, body, operation_key)
    if kind in {"sum", "count", "product"}:
        var = {"sum": "total", "count": "count", "product": "product"}[kind]
        update = {
            "sum": f"total += {term};",
            "count": "count += 1;",
            "product": f"product *= {term};",
        }[kind]
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  int {var} = {init};
  {iter_head} {{
    if ({pred}) {{
      {update}
    }}{inc}
  }}
  return {var};
}}
"""
    elif kind == "any":
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  {iter_head} {{
    if ({pred}) {{
      return 1;
    }}{inc}
  }}
  return 0;
}}
"""
    elif kind == "all":
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  {iter_head} {{
    if ({negate(pred, "c")}) {{
      return 0;
    }}{inc}
  }}
  return 1;
}}
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        body = f"""int {operation_key}(int *xs, int n) {{{prefix}
  int best = {init};
  {iter_head} {{
    if (xs[i] {cmp} best) {{
      best = xs[i];
    }}{inc}
  }}
  return best;
}}
"""
    else:
        raise ValueError(kind)
    return Variant(representation, body, operation_key)


def ruby_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    op = OPERATIONS[operation_key]
    if op.arity == 2:
        return ruby_pair_variant(operation_key, representation, negative)
    kind, predicate_key, init, contribution = effective_components(op, negative)
    pred = render_predicate(predicate_key, "x")
    term = render_contribution(contribution, "x")
    if kind == "sum_abs":
        if representation in {"loop", "indexed_loop"}:
            loop_head = "xs.each do |x|" if representation == "loop" else "while i < xs.length"
            loop_prefix = "" if representation == "loop" else "  i = 0\n"
            loop_bind = "" if representation == "loop" else "\n    x = xs[i]"
            loop_inc = "" if representation == "loop" else "\n    i += 1"
            loop_end = "end"
            src = f"""def {operation_key}(xs)
  total = {init}
{loop_prefix}  {loop_head}{loop_bind}
    if x < 0
      total += -x
    else
      total += x
    end{loop_inc}
  {loop_end}
  total
end
"""
        else:
            src = f"""def {operation_key}(xs)
  xs.reduce({init}) {{ |total, x| total + (x < 0 ? -x : x) }}
end
"""
        return Variant(representation, src, operation_key)
    if representation in {"loop", "indexed_loop"}:
        loop_head = "xs.each do |x|" if representation == "loop" else "while i < xs.length"
        loop_prefix = "" if representation == "loop" else "  i = 0\n"
        loop_bind = "" if representation == "loop" else "\n    x = xs[i]"
        loop_inc = "" if representation == "loop" else "\n    i += 1"
        loop_end = "end"
        if kind in {"sum", "count", "product"}:
            var = {"sum": "total", "count": "count", "product": "product"}[kind]
            update = {"sum": f"total += {term}", "count": "count += 1", "product": f"product *= {term}"}[
                kind
            ]
            src = f"""def {operation_key}(xs)
  {var} = {init}
{loop_prefix}  {loop_head}{loop_bind}
    if {pred}
      {update}
    end{loop_inc}
  {loop_end}
  {var}
end
"""
        elif kind == "any":
            src = f"""def {operation_key}(xs)
{loop_prefix}  {loop_head}{loop_bind}
    if {pred}
      return true
    end{loop_inc}
  {loop_end}
  false
end
"""
        elif kind == "all":
            src = f"""def {operation_key}(xs)
{loop_prefix}  {loop_head}{loop_bind}
    if {negate(pred, "ruby")}
      return false
    end{loop_inc}
  {loop_end}
  true
end
"""
        elif kind in {"max", "min"}:
            cmp = ">" if kind == "max" else "<"
            src = f"""def {operation_key}(xs)
  best = {init}
{loop_prefix}  {loop_head}{loop_bind}
    if x {cmp} best
      best = x
    end{loop_inc}
  {loop_end}
  best
end
"""
        else:
            raise ValueError(kind)
    elif kind == "sum":
        src = f"""def {operation_key}(xs)
  xs.select {{ |x| {pred} }}.reduce({init}) {{ |total, x| total + {term} }}
end
"""
    elif kind == "count":
        src = f"""def {operation_key}(xs)
  xs.count {{ |x| {pred} }}
end
"""
    elif kind == "any":
        src = f"""def {operation_key}(xs)
  xs.any? {{ |x| {pred} }}
end
"""
    elif kind == "all":
        src = f"""def {operation_key}(xs)
  xs.all? {{ |x| {pred} }}
end
"""
    elif kind == "product":
        src = f"""def {operation_key}(xs)
  xs.select {{ |x| {pred} }}.reduce({init}) {{ |product, x| product * {term} }}
end
"""
    elif kind in {"max", "min"}:
        cmp = ">" if kind == "max" else "<"
        src = f"""def {operation_key}(xs)
  xs.reduce({init}) {{ |best, x| x {cmp} best ? x : best }}
end
"""
    else:
        raise ValueError(kind)
    return Variant(representation, src, operation_key)


def python_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    indexed_term = render_contribution(contribution, "a[i]", "b[i]")
    pair_term = render_contribution(contribution, "x", "y")
    if representation == "loop":
        src = f"""def {operation_key}(a, b):
    total = {init}
    for i in range(len(a)):
        total += {indexed_term}
    return total
"""
    else:
        prefix = "" if init == 0 else f"{init} + "
        src = f"""def {operation_key}(a, b):
    return {prefix}sum({pair_term} for x, y in zip(a, b))
"""
    return Variant(representation, src, operation_key)


def js_pair_variant(surface: Surface, operation_key: str, representation: str, negative: bool) -> Variant:
    name = snake_to_camel(operation_key)
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    indexed_term = render_contribution(contribution, "a[i]", "b[i]")
    if representation == "loop":
        body = f"""function {name}(a, b) {{
  let total = {init};
  for (let i = 0; i < a.length; i += 1) {{
    total += {indexed_term};
  }}
  return total;
}}
"""
    else:
        body = f"""function {name}(a, b) {{
  let total = {init};
  let i = 0;
  while (i < a.length) {{
    total += {indexed_term};
    i += 1;
  }}
  return total;
}}
"""
    return Variant(representation, js_script_wrap(surface, body), name, js_start_line(surface))


def typescript_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    v = js_pair_variant(Surface("typescript", "typescript", "ts"), operation_key, representation, negative)
    name = snake_to_camel(operation_key)
    src = v.source.replace(f"function {name}(a, b)", f"function {name}(a: number[], b: number[]): number")
    return Variant(representation, src, name)


def go_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    name = snake_to_pascal(operation_key)
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    if representation == "loop":
        term = render_contribution(contribution, "x", "b[i]")
        body = f"""package main

func {name}(a []int, b []int) int {{
	total := {init}
	for i, x := range a {{
		total += {term}
	}}
	return total
}}
"""
    else:
        term = render_contribution(contribution, "a[i]", "b[i]")
        body = f"""package main

func {name}(a []int, b []int) int {{
	total := {init}
	for i := 0; i < len(a); i++ {{
		total += {term}
	}}
	return total
}}
"""
    return Variant(representation, body, name)


def rust_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    if representation == "loop":
        term = render_contribution(contribution, "a[i]", "b[i]")
        src = f"""pub fn {operation_key}(a: &[i32], b: &[i32]) -> i32 {{
    let mut total = {init};
    for i in 0..a.len() {{
        total += {term};
    }}
    total
}}
"""
    else:
        term = render_contribution(contribution, "*x", "*y")
        src = f"""pub fn {operation_key}(a: &[i32], b: &[i32]) -> i32 {{
    a.iter().zip(b.iter()).fold({init}, |total, (x, y)| total + {term})
}}
"""
    return Variant(representation, src, operation_key)


def java_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    name = snake_to_camel(operation_key)
    class_name = f"Case{snake_to_pascal(operation_key)}{representation.title()}"
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    term = render_contribution(contribution, "a[i]", "b[i]")
    if representation == "loop":
        body = f"""class {class_name} {{
  static int {name}(int[] a, int[] b) {{
    int total = {init};
    for (int i = 0; i < a.length; i++) {{
      total += {term};
    }}
    return total;
  }}
}}
"""
    else:
        body = f"""class {class_name} {{
  static int {name}(int[] a, int[] b) {{
    int total = {init};
    int i = 0;
    while (i < a.length) {{
      total += {term};
      i++;
    }}
    return total;
  }}
}}
"""
    return Variant(representation, body, name)


def c_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    term = render_contribution(contribution, "a[i]", "b[i]")
    if representation == "loop":
        iter_head = "for (int i = 0; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_start_one":
        iter_head = "for (int i = 1; i < n; i++)"
        inc = ""
        prefix = ""
    elif representation == "c_stride_two":
        iter_head = "for (int i = 0; i < n; i += 2)"
        inc = ""
        prefix = ""
    elif representation == "aggregate":
        iter_head = "while (i < n)"
        inc = "\n    i++;"
        prefix = "\n  int i = 0;"
    else:
        raise ValueError(f"unknown C representation: {representation}")
    src = f"""int {operation_key}(int *a, int *b, int n) {{{prefix}
  int total = {init};
  {iter_head} {{
    total += {term};{inc}
  }}
  return total;
}}
"""
    return Variant(representation, src, operation_key)


def ruby_pair_variant(operation_key: str, representation: str, negative: bool) -> Variant:
    _, _, init, contribution = effective_components(OPERATIONS[operation_key], negative)
    if representation == "loop":
        term = render_contribution(contribution, "x", "b[i]")
        src = f"""def {operation_key}(a, b)
  total = {init}
  a.each_with_index do |x, i|
    total += {term}
  end
  total
end
"""
    else:
        term = render_contribution(contribution, "a[i]", "b[i]")
        src = f"""def {operation_key}(a, b)
  total = {init}
  i = 0
  while i < a.length
    total += {term}
    i += 1
  end
  total
end
"""
    return Variant(representation, src, operation_key)


EMITTERS = {
    "python": python_variant,
    "javascript": lambda op, rep, neg: js_variant(Surface("javascript", "javascript", "js"), op, rep, neg),
    "typescript": typescript_variant,
    "go": go_variant,
    "rust": rust_variant,
    "java": java_variant,
    "c": c_variant,
    "ruby": ruby_variant,
    "vue": lambda op, rep, neg: js_variant(Surface("vue", "javascript", "vue", "script"), op, rep, neg),
    "svelte": lambda op, rep, neg: js_variant(Surface("svelte", "javascript", "svelte", "script"), op, rep, neg),
    "html": lambda op, rep, neg: js_variant(Surface("html", "javascript", "html", "script"), op, rep, neg),
}
