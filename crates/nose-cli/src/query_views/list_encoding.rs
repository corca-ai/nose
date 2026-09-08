//! Encode independent list rows in parallel, preserving serde_json's exact bytes.
use rayon::prelude::*;
use serde_json::Value;

pub(super) fn encode(value: &Value) -> serde_json::Result<String> {
    let Some(object) = value.as_object() else {
        return serde_json::to_string(value);
    };
    let Some(families) = object
        .get("families")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() >= 64)
    else {
        return serde_json::to_string(value);
    };
    let chunks = families
        .par_chunks(32)
        .map(|chunk| {
            let mut bytes = Vec::new();
            for (index, row) in chunk.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                serde_json::to_writer(&mut bytes, row)?;
            }
            Ok(bytes)
        })
        .collect::<serde_json::Result<Vec<_>>>()?;
    let mut output = Vec::with_capacity(chunks.iter().map(Vec::len).sum());
    output.push(b'{');
    for (index, (key, field)) in object.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        serde_json::to_writer(&mut output, key)?;
        output.push(b':');
        if key == "families" {
            output.push(b'[');
            for (index, chunk) in chunks.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                output.extend_from_slice(chunk);
            }
            output.push(b']');
        } else {
            serde_json::to_writer(&mut output, field)?;
        }
    }
    output.push(b'}');
    Ok(String::from_utf8(output).expect("JSON encoding is UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_chunks_match_serde_bytes_across_boundaries_and_thread_counts() {
        for threads in [1, 3] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for count in [0, 1, 31, 32, 63, 64, 65, 257] {
                let families = (0..count)
                    .map(|index| {
                        serde_json::json!({
                            "id": index, "text": "한글\n\t\"\\", "empty": {}, "rows": [],
                            "score": -0.0, "large": u64::MAX, "missing": null,
                            "nested": [{"families": [false, true, 1.25e-100]}]
                        })
                    })
                    .collect::<Vec<_>>();
                let value =
                    serde_json::json!({"before": [1, 2], "families": families, "z": "\u{0}"});
                assert_eq!(
                    pool.install(|| encode(&value)).unwrap(),
                    serde_json::to_string(&value).unwrap()
                );
            }
        }
        for value in [
            Value::Null,
            serde_json::json!({"families": {}}),
            serde_json::json!(["plain"]),
        ] {
            assert_eq!(
                encode(&value).unwrap(),
                serde_json::to_string(&value).unwrap()
            );
        }
    }
}
