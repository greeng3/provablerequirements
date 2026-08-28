//! Typed parse of `SystemConfig.llm` per `LLM-configSchema`.
//!
//! The SystemConfig's opaque `llm: Option<serde_json::Value>`
//! gets normalised here into a priority-ordered
//! `Vec<ProviderConfig>`. Unknown provider families surface
//! as a typed error naming the offending index so operators
//! can pinpoint which entry is wrong.

use serde::{Deserialize, Serialize};

use crate::schema::SystemConfig;

/// One entry from the `SystemConfig.llm` array. Priority order
/// is the array order — `run_chain` walks top-to-bottom, skipping
/// disabled entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub provider: ProviderFamily,
    pub model: String,
    /// OpenAI-compatible: required (points at the provider's
    /// base URL). Anthropic / Gemini: optional; the adapter
    /// falls back to the published default when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// API key the adapter sends. `None` for keyless providers
    /// (e.g. a local Ollama instance behind the OpenAI-compatible
    /// adapter). Stored in the System config under `apiKey` per
    /// LLM-secretsViaEnv (which after Phase 13 stores keys in
    /// the System config rather than via env vars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// `false` removes the entry from the fallback chain at
    /// load time. Conceptual default is `true`; the field is
    /// optional so System configs that don't care about
    /// per-entry on/off don't have to carry the noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl ProviderConfig {
    /// Effective `enabled` flag (conceptual default: `true`).
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

/// One of the three adapter families shipped in 10a. Future
/// additions are add-only — existing System configs continue
/// to parse when a new family is introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderFamily {
    OpenaiCompatible,
    Anthropic,
    Gemini,
}

impl ProviderFamily {
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::OpenaiCompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("`llm` must be an array")]
    NotAnArray,
    #[error("`llm[{index}]` is not an object")]
    NotAnObject { index: usize },
    #[error("`llm[{index}].{field}` is required")]
    MissingField { index: usize, field: &'static str },
    #[error("`llm[{index}].{field}` must be a string")]
    WrongType { index: usize, field: &'static str },
    #[error(
        "`llm[{index}].provider` has unknown value '{value}' — expected one of \
         {{openai-compatible, anthropic, gemini}}"
    )]
    UnknownProvider { index: usize, value: String },
    #[error("`llm[{index}].endpoint` is required for the {family} adapter")]
    EndpointRequired { index: usize, family: &'static str },
}

/// Parse the opaque `SystemConfig.llm` value into a priority-
/// ordered list. An empty or missing array yields an empty
/// `Vec` per `LLM-optional`.
pub fn parse_llm(raw: Option<&serde_json::Value>) -> Result<Vec<ProviderConfig>, ParseError> {
    let Some(value) = raw else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let array = value.as_array().ok_or(ParseError::NotAnArray)?;
    let mut out = Vec::with_capacity(array.len());
    for (index, raw) in array.iter().enumerate() {
        out.push(parse_entry(index, raw)?);
    }
    Ok(out)
}

/// Convenience for callers who hold a `&SystemConfig` directly.
pub fn llm_config(config: &SystemConfig) -> Result<Vec<ProviderConfig>, ParseError> {
    parse_llm(config.llm.as_ref())
}

fn parse_entry(index: usize, raw: &serde_json::Value) -> Result<ProviderConfig, ParseError> {
    let obj = raw.as_object().ok_or(ParseError::NotAnObject { index })?;
    let provider_raw =
        obj.get("provider")
            .and_then(|v| v.as_str())
            .ok_or(ParseError::MissingField {
                index,
                field: "provider",
            })?;
    let family = match provider_raw {
        "openai-compatible" => ProviderFamily::OpenaiCompatible,
        "anthropic" => ProviderFamily::Anthropic,
        "gemini" => ProviderFamily::Gemini,
        other => {
            return Err(ParseError::UnknownProvider {
                index,
                value: other.to_owned(),
            });
        }
    };
    let model = obj
        .get("model")
        .and_then(|v| v.as_str())
        .ok_or(ParseError::MissingField {
            index,
            field: "model",
        })?
        .to_owned();
    let endpoint = match obj.get("endpoint") {
        Some(serde_json::Value::Null) | None => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(_) => {
            return Err(ParseError::WrongType {
                index,
                field: "endpoint",
            });
        }
    };
    // OpenAI-compatible demands an endpoint — it's the whole
    // point of the adapter (pointing at OpenAI vs Ollama vs
    // LMStudio vs …).
    if matches!(family, ProviderFamily::OpenaiCompatible) && endpoint.is_none() {
        return Err(ParseError::EndpointRequired {
            index,
            family: "openai-compatible",
        });
    }
    let api_key = match obj.get("apiKey") {
        Some(serde_json::Value::Null) | None => None,
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                None
            } else {
                Some(s.clone())
            }
        }
        Some(_) => {
            return Err(ParseError::WrongType {
                index,
                field: "apiKey",
            });
        }
    };
    let enabled = match obj.get("enabled") {
        Some(serde_json::Value::Null) | None => None,
        Some(serde_json::Value::Bool(b)) => Some(*b),
        Some(_) => {
            return Err(ParseError::WrongType {
                index,
                field: "enabled",
            });
        }
    };
    Ok(ProviderConfig {
        provider: family,
        model,
        endpoint,
        api_key,
        enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_llm_yields_empty_list() {
        assert!(parse_llm(None).unwrap().is_empty());
    }

    #[test]
    fn null_llm_yields_empty_list() {
        let v = serde_json::Value::Null;
        assert!(parse_llm(Some(&v)).unwrap().is_empty());
    }

    #[test]
    fn empty_array_yields_empty_list() {
        let v = serde_json::json!([]);
        assert!(parse_llm(Some(&v)).unwrap().is_empty());
    }

    #[test]
    fn openai_compatible_entry_with_endpoint_and_key_parses() {
        let v = serde_json::json!([
            {
                "provider": "openai-compatible",
                "model": "gpt-4o-mini",
                "endpoint": "https://api.openai.com",
                "apiKey": "sk-test-1234",
            }
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].provider, ProviderFamily::OpenaiCompatible);
        assert_eq!(parsed[0].model, "gpt-4o-mini");
        assert_eq!(
            parsed[0].endpoint.as_deref(),
            Some("https://api.openai.com")
        );
        assert_eq!(parsed[0].api_key.as_deref(), Some("sk-test-1234"));
        // Conceptual default: enabled.
        assert!(parsed[0].is_enabled());
    }

    #[test]
    fn anthropic_entry_parses_without_endpoint() {
        let v = serde_json::json!([
            { "provider": "anthropic", "model": "claude-haiku-4-5", "apiKey": "sk-ant-1234" }
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert_eq!(parsed[0].provider, ProviderFamily::Anthropic);
        assert!(parsed[0].endpoint.is_none());
        assert_eq!(parsed[0].api_key.as_deref(), Some("sk-ant-1234"));
    }

    #[test]
    fn gemini_entry_parses_without_endpoint() {
        let v = serde_json::json!([
            { "provider": "gemini", "model": "gemini-2.0-flash", "apiKey": "AIza-test" }
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert_eq!(parsed[0].provider, ProviderFamily::Gemini);
        assert_eq!(parsed[0].api_key.as_deref(), Some("AIza-test"));
    }

    #[test]
    fn keyless_openai_compatible_entry_parses() {
        // Local Ollama: endpoint required, apiKey omitted.
        let v = serde_json::json!([
            {
                "provider": "openai-compatible",
                "model": "qwen2.5-coder:14b",
                "endpoint": "http://host.docker.internal:11434",
            }
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert_eq!(parsed[0].provider, ProviderFamily::OpenaiCompatible);
        assert!(parsed[0].api_key.is_none());
    }

    #[test]
    fn empty_string_api_key_normalises_to_none() {
        let v = serde_json::json!([
            {
                "provider": "openai-compatible",
                "model": "x",
                "endpoint": "http://x.test",
                "apiKey": "",
            }
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert!(parsed[0].api_key.is_none());
    }

    #[test]
    fn explicit_disabled_flag_is_observed() {
        let v = serde_json::json!([
            {
                "provider": "openai-compatible",
                "model": "x",
                "endpoint": "http://x.test",
                "enabled": false,
            }
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert_eq!(parsed[0].enabled, Some(false));
        assert!(!parsed[0].is_enabled());
    }

    #[test]
    fn openai_compatible_without_endpoint_is_an_error() {
        let v = serde_json::json!([
            { "provider": "openai-compatible", "model": "gpt-4o-mini" }
        ]);
        let err = parse_llm(Some(&v)).unwrap_err();
        assert!(matches!(err, ParseError::EndpointRequired { index: 0, .. }));
    }

    #[test]
    fn unknown_provider_reports_index_and_value() {
        let v = serde_json::json!([
            { "provider": "some-random-llm", "model": "x" }
        ]);
        let err = parse_llm(Some(&v)).unwrap_err();
        assert!(matches!(
            err,
            ParseError::UnknownProvider { index: 0, ref value } if value == "some-random-llm"
        ));
    }

    #[test]
    fn missing_model_reports_index() {
        let v = serde_json::json!([
            { "provider": "anthropic" }
        ]);
        let err = parse_llm(Some(&v)).unwrap_err();
        assert!(matches!(
            err,
            ParseError::MissingField {
                index: 0,
                field: "model"
            }
        ));
    }

    #[test]
    fn non_array_root_is_an_error() {
        let v = serde_json::json!({ "provider": "anthropic", "model": "x" });
        let err = parse_llm(Some(&v)).unwrap_err();
        assert!(matches!(err, ParseError::NotAnArray));
    }

    #[test]
    fn priority_order_is_the_array_order() {
        let v = serde_json::json!([
            { "provider": "anthropic", "model": "claude-haiku-4-5" },
            { "provider": "openai-compatible", "model": "gpt-4o-mini", "endpoint": "https://x.test" },
            { "provider": "gemini", "model": "gemini-2.0-flash" },
        ]);
        let parsed = parse_llm(Some(&v)).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].provider, ProviderFamily::Anthropic);
        assert_eq!(parsed[1].provider, ProviderFamily::OpenaiCompatible);
        assert_eq!(parsed[2].provider, ProviderFamily::Gemini);
    }
}
