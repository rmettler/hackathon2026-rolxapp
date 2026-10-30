/// Redacts the values of given `sensitive_keys` if direct key of `value`. Disabled by the
/// `uncensored_logs` feature.
#[cfg(not(feature = "uncensored_logs"))]
pub fn redact_json(mut value: serde_json::Value, sensitive_keys: &[&str]) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        for &key in sensitive_keys {
            if obj.contains_key(key) {
                obj.insert(key.into(), serde_json::Value::String("***".into()));
            }
        }
    }
    value
}

/// Returns value as is, the `uncensored_logs` feature is on.
#[cfg(feature = "uncensored_logs")]
pub fn redact_json(value: serde_json::Value, _sensitive_keys: &[&str]) -> serde_json::Value {
    value
}

#[cfg(all(test, not(feature = "uncensored_logs")))]
mod tests {
    use super::*;

    #[test]
    fn redacts_only_the_given_keys() {
        let value = serde_json::json!({ "bearerToken": "secret", "status": 1 });
        assert_eq!(
            redact_json(value, &["bearerToken", "absent"]),
            serde_json::json!({ "bearerToken": "***", "status": 1 })
        );
    }
}
