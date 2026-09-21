// Shared test helpers. Cargo compiles this module into EVERY integration
// test binary, so an item only one binary uses is dead code in the
// others; the allow keeps that from being a warning rather than hiding
// anything real.
#![allow(dead_code)]

/// The engine's value as plain JSON: the same normalisation the
/// TypeScript suite's `plain()` and the Go suite's `plain()` apply, so a
/// result compares by shape rather than by container representation.
pub fn plain(value: &tabnas::Value) -> serde_json::Value {
    whole(value.to_json())
}

/// Numbers with no fractional part as integers, the way a JSON round-trip
/// renders them. The engine holds every number as an `f64`, so its
/// `to_json` yields `1.0` where `serde_json::json!` yields `1`, and the
/// two never compare equal without this.
pub fn whole(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Number(number) => match number.as_f64() {
            Some(float) if float.fract() == 0.0 && float.abs() < 9.0e15 => {
                Value::Number((float as i64).into())
            }
            _ => Value::Number(number),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(whole).collect()),
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key, whole(value)))
                .collect(),
        ),
        other => other,
    }
}

/// The records of a parsed document, failing loudly when the result is
/// not an array: a JSON Lines document always parses to one.
pub fn records(value: &tabnas::Value) -> Vec<serde_json::Value> {
    match plain(value) {
        serde_json::Value::Array(items) => items,
        other => panic!("expected an array of records, got {other}"),
    }
}
