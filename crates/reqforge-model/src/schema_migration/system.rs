//! System-config migration chain.
//!
//! v1 → v2 (Phase 13): drop `apiKeyEnvVar` from each `llm[]`
//! entry, read the named env var once at upgrade time, and
//! write the value into a new `apiKey` field. Operators who
//! ran on env-var-managed keys land in the new world after one
//! migration run; the runtime no longer reads env vars for
//! keys after this step.

use serde_json::Value;

use super::registry::{FileType, MigrationStep, Registry};

pub const SYSTEM_STEPS: &[MigrationStep] = &[v1_to_v2_drop_api_key_env_var];

pub fn build() -> Registry {
    Registry::new(FileType::System, 1, SYSTEM_STEPS.to_vec())
}

/// v1 → v2: each provider entry's `apiKeyEnvVar` (if present)
/// is consumed by reading the named env var. If the env var is
/// set, its value lands in a new `apiKey` field. Either way,
/// `apiKeyEnvVar` is removed. Entries that already have an
/// `apiKey` (operator hand-edited or the system was authored
/// post-Phase-13) keep theirs unchanged.
fn v1_to_v2_drop_api_key_env_var(mut value: Value) -> Result<Value, String> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "system config root must be an object".to_owned())?;
    if let Some(llm_value) = obj.get_mut("llm")
        && let Some(arr) = llm_value.as_array_mut()
    {
        for entry in arr {
            if let Some(entry_obj) = entry.as_object_mut() {
                let env_var = entry_obj.remove("apiKeyEnvVar");
                if entry_obj.contains_key("apiKey") {
                    // Operator already has an apiKey — leave it
                    // alone, just drop the legacy field.
                    continue;
                }
                if let Some(Value::String(name)) = env_var
                    && let Ok(secret) = std::env::var(&name)
                {
                    entry_obj.insert("apiKey".to_owned(), Value::String(secret));
                }
                // env unset: silently drop the field. The
                // provider becomes keyless after the migration;
                // the operator can re-enter the key on /llm.
            }
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_step(value: Value) -> Value {
        v1_to_v2_drop_api_key_env_var(value).unwrap()
    }

    #[test]
    fn migration_copies_env_var_value_into_api_key_when_set() {
        // SAFETY: unique env-var name per test; single-threaded
        // test isolation.
        unsafe { std::env::set_var("REQFORGE_TEST_MIG_KEY_A", "sk-from-env") };
        let v = json!({
            "schemaVersion": 1,
            "llm": [
                {
                    "provider": "openai-compatible",
                    "model": "gpt-4o-mini",
                    "endpoint": "https://x.test",
                    "apiKeyEnvVar": "REQFORGE_TEST_MIG_KEY_A"
                }
            ]
        });
        let out = run_step(v);
        let entry = &out["llm"][0];
        assert_eq!(entry["apiKey"], "sk-from-env");
        assert!(entry.get("apiKeyEnvVar").is_none());
    }

    #[test]
    fn migration_drops_env_var_field_when_env_is_unset() {
        unsafe { std::env::remove_var("REQFORGE_TEST_MIG_KEY_B") };
        let v = json!({
            "schemaVersion": 1,
            "llm": [
                {
                    "provider": "anthropic",
                    "model": "claude-haiku-4-5",
                    "apiKeyEnvVar": "REQFORGE_TEST_MIG_KEY_B"
                }
            ]
        });
        let out = run_step(v);
        let entry = &out["llm"][0];
        assert!(entry.get("apiKey").is_none());
        assert!(entry.get("apiKeyEnvVar").is_none());
    }

    #[test]
    fn migration_preserves_existing_api_key() {
        unsafe { std::env::set_var("REQFORGE_TEST_MIG_KEY_C", "should-not-overwrite") };
        let v = json!({
            "schemaVersion": 1,
            "llm": [
                {
                    "provider": "anthropic",
                    "model": "claude-haiku-4-5",
                    "apiKey": "existing-key",
                    "apiKeyEnvVar": "REQFORGE_TEST_MIG_KEY_C"
                }
            ]
        });
        let out = run_step(v);
        let entry = &out["llm"][0];
        assert_eq!(entry["apiKey"], "existing-key");
        assert!(entry.get("apiKeyEnvVar").is_none());
    }

    #[test]
    fn migration_is_a_noop_for_systems_with_no_llm_block() {
        let v = json!({ "schemaVersion": 1, "name": "p" });
        let out = run_step(v.clone());
        assert_eq!(out, v);
    }

    #[test]
    fn migration_handles_keyless_entries() {
        // No apiKeyEnvVar, no apiKey — Ollama-style keyless.
        let v = json!({
            "schemaVersion": 1,
            "llm": [
                {
                    "provider": "openai-compatible",
                    "model": "qwen2.5-coder:14b",
                    "endpoint": "http://host.docker.internal:11434"
                }
            ]
        });
        let out = run_step(v.clone());
        assert_eq!(out, v);
    }
}
