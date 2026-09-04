use super::*;

// ECMAScript SameValueZero on the primitive portion the oracle can represent.
fn key_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Str(a), Value::Str(b)) => {
            let empty = stable_symbol_hash("");
            a.first().unwrap_or(&empty) == b.first().unwrap_or(&empty)
        }
        (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => *a as f64 == b.0,
        _ => left == right,
    }
}

fn concrete_key(value: &Value) -> bool {
    matches!(value, Value::Int(_) | Value::Float(_) | Value::Bool(_))
        || matches!(value, Value::Str(pieces) if pieces.len() <= 1)
}

/// Build a key-only collection under SameValueZero. Multi-piece symbolic strings
/// cannot establish character equality with an atomic key and stay unsupported.
pub fn keyed_membership_value(keys: Vec<Value>) -> Option<Value> {
    if !keys.iter().all(concrete_key) {
        return None;
    }
    let mut unique = Vec::new();
    for key in keys {
        if !unique.iter().any(|old| key_equal(old, &key)) {
            unique.push(key);
        }
    }
    Some(Value::KeySet(unique))
}

pub(super) fn bind(value: Value, key: nose_il::DomainEvidence) -> Value {
    let opaque = Value::Sym(sym_id(0x4b45_5953, &[vhash(&value)]));
    if contains_sym(&value) {
        return opaque;
    }
    let keys = match value {
        Value::KeySet(keys) | Value::List(keys) => keys,
        Value::Null => Vec::new(),
        value => vec![value],
    };
    keyed_membership_value(
        keys.into_iter()
            .map(|v| coerce_to_declared_domain(v, key))
            .collect(),
    )
    .unwrap_or(opaque)
}

pub(super) fn contains(keys: &[Value], needle: &Value) -> R<Value> {
    if !concrete_key(needle) {
        return Err(Unsupported::protocol("protocol.key-equality-unmodeled"));
    }
    Ok(Value::Bool(keys.iter().any(|key| key_equal(key, needle))))
}
