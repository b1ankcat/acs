/// Metadata for a single provider field.
use std::collections::HashMap;

pub struct FieldDef {
    /// Internal config key (e.g. "ANTHROPIC_BASE_URL")
    pub key: &'static str,
    /// CLI arg name without `--` (e.g. "base-url"), used for HashMap lookup from CLI args
    pub arg: &'static str,
    /// Whether this field is required when adding a provider
    pub required: bool,
    /// Whether to mask value in display (API keys, tokens)
    pub secret: bool,
    /// Fixed default value written automatically (never prompted); None = user-provided
    pub default: Option<&'static str>,
    /// Whether a defaulted field can be overridden by add/config input.
    pub editable_default: bool,
    /// If true, value is set to the provider name automatically (codex model_provider)
    pub from_name: bool,
}

pub const DEFAULT_MODEL_CONTEXT_WINDOW: &str = "1000000";
pub const DEFAULT_MODEL_AUTO_COMPACT_TOKEN_LIMIT: &str = "900000";

impl FieldDef {
    /// Returns true if this field should appear in interactive prompts (not auto-filled).
    pub fn is_promptable(&self) -> bool {
        !self.from_name && (self.default.is_none() || self.editable_default)
    }
}

pub static CLAUDE_FIELDS: &[FieldDef] = &[
    FieldDef { key: "ANTHROPIC_BASE_URL", arg: "base-url", required: true, secret: false, default: None, editable_default: false, from_name: false },
    FieldDef { key: "ANTHROPIC_AUTH_TOKEN", arg: "api-key", required: false, secret: true, default: None, editable_default: false, from_name: false },
    FieldDef { key: "ANTHROPIC_MODEL", arg: "model", required: false, secret: false, default: None, editable_default: false, from_name: false },
    FieldDef { key: "CLAUDE_CODE_SUBAGENT_MODEL", arg: "subagent-model", required: false, secret: false, default: None, editable_default: false, from_name: false },
    FieldDef { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL", arg: "haiku-model", required: false, secret: false, default: None, editable_default: false, from_name: false },
    FieldDef { key: "ANTHROPIC_DEFAULT_SONNET_MODEL", arg: "sonnet-model", required: false, secret: false, default: None, editable_default: false, from_name: false },
    FieldDef { key: "ANTHROPIC_DEFAULT_OPUS_MODEL", arg: "opus-model", required: false, secret: false, default: None, editable_default: false, from_name: false },
];

pub static CODEX_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "base_url", arg: "base-url", required: true, secret: false,
        default: None, editable_default: false, from_name: false,
    },
    FieldDef {
        key: "openai_api_key", arg: "api-key", required: false, secret: true,
        default: None, editable_default: false, from_name: false,
    },
    FieldDef {
        key: "model", arg: "model", required: false, secret: false,
        default: None, editable_default: false, from_name: false,
    },
    FieldDef {
        key: "model_reasoning_effort", arg: "reasoning-effort", required: false, secret: false,
        default: None, editable_default: false, from_name: false,
    },
    FieldDef {
        key: "model_context_window", arg: "model-context-window", required: false, secret: false,
        default: Some(DEFAULT_MODEL_CONTEXT_WINDOW), editable_default: true, from_name: false,
    },
    FieldDef {
        key: "model_auto_compact_token_limit", arg: "model-auto-compact-token-limit", required: false, secret: false,
        default: Some(DEFAULT_MODEL_AUTO_COMPACT_TOKEN_LIMIT), editable_default: true, from_name: false,
    },
    FieldDef {
        key: "model_provider", arg: "", required: false, secret: false,
        default: None, editable_default: false, from_name: true,
    },
    FieldDef {
        key: "disable_response_storage", arg: "", required: false, secret: false,
        default: Some("true"), editable_default: false, from_name: false,
    },
    FieldDef {
        key: "requires_openai_auth", arg: "", required: false, secret: false,
        default: Some("true"), editable_default: false, from_name: false,
    },
    FieldDef {
        key: "wire_api", arg: "", required: false, secret: false,
        default: Some("responses"), editable_default: false, from_name: false,
    },
];

pub static GEMINI_FIELDS: &[FieldDef] = &[
    FieldDef { key: "GOOGLE_GEMINI_BASE_URL", arg: "base-url", required: true, secret: false, default: None, editable_default: false, from_name: false },
    FieldDef { key: "GEMINI_API_KEY", arg: "api-key", required: false, secret: true, default: None, editable_default: false, from_name: false },
    FieldDef { key: "GEMINI_MODEL", arg: "model", required: false, secret: false, default: None, editable_default: false, from_name: false },
];

pub fn fields_for(tool_name: &str) -> &'static [FieldDef] {
    match tool_name {
        "claude" => CLAUDE_FIELDS,
        "codex"  => CODEX_FIELDS,
        "gemini" => GEMINI_FIELDS,
        _        => panic!("unknown tool (programmer error): {}", tool_name),
    }
}

pub fn validate_provider_fields(
    tool_name: &str,
    values: &HashMap<String, String>,
) -> Result<(), String> {
    if tool_name != "codex" {
        return Ok(());
    }

    let context_window = parse_positive_integer(
        "model_context_window",
        values
            .get("model_context_window")
            .map(String::as_str)
            .unwrap_or(DEFAULT_MODEL_CONTEXT_WINDOW),
    )?;
    let auto_compact_limit = parse_positive_integer(
        "model_auto_compact_token_limit",
        values
            .get("model_auto_compact_token_limit")
            .map(String::as_str)
            .unwrap_or(DEFAULT_MODEL_AUTO_COMPACT_TOKEN_LIMIT),
    )?;
    if auto_compact_limit >= context_window {
        return Err(
            "model_auto_compact_token_limit must be less than model_context_window".to_string(),
        );
    }
    Ok(())
}

fn parse_positive_integer(key: &str, value: &str) -> Result<i64, String> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| format!("{key} must be a positive integer"))?;
    if parsed <= 0 {
        return Err(format!("{key} must be a positive integer"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_each_tool_has_required_base_url() {
        for tool in ["claude", "codex", "gemini"] {
            let fields = fields_for(tool);
            assert!(
                fields.iter().any(|f| f.arg == "base-url" && f.required),
                "{tool} should have a required base-url field"
            );
        }
    }

    #[test]
    fn test_each_tool_has_api_key_secret() {
        for tool in ["claude", "codex", "gemini"] {
            let fields = fields_for(tool);
            assert!(
                fields.iter().any(|f| f.arg == "api-key" && f.secret),
                "{tool} should have a secret api-key field"
            );
        }
    }

    #[test]
    fn test_promptable_excludes_auto_fields() {
        for f in CODEX_FIELDS {
            if (f.default.is_some() && !f.editable_default) || f.from_name {
                assert!(!f.is_promptable(), "auto field '{}' should not be promptable", f.key);
            }
        }
    }

    #[test]
    fn test_codex_has_auto_defaults() {
        let keys: Vec<_> = CODEX_FIELDS.iter().filter(|f| f.default.is_some()).map(|f| f.key).collect();
        assert!(keys.contains(&"disable_response_storage"));
        assert!(keys.contains(&"requires_openai_auth"));
        assert!(keys.contains(&"wire_api"));
        assert!(keys.contains(&"model_context_window"));
        assert!(keys.contains(&"model_auto_compact_token_limit"));
    }

    #[test]
    fn test_non_auto_optional_fields_have_nonempty_arg() {
        for tool in ["claude", "codex", "gemini"] {
            for f in fields_for(tool) {
                if !f.from_name && f.default.is_none() {
                    assert!(!f.arg.is_empty(),
                        "field '{}' in {tool} is not auto-filled but has empty arg", f.key);
                }
            }
        }
    }

    #[test]
    fn test_validate_codex_context_limits() {
        let values = HashMap::new();
        assert!(validate_provider_fields("codex", &values).is_ok());

        let values = [
            ("model_context_window".to_string(), "0".to_string()),
        ]
        .into();
        assert!(validate_provider_fields("codex", &values).is_err());

        let values = [
            ("model_context_window".to_string(), "900000".to_string()),
            ("model_auto_compact_token_limit".to_string(), "900000".to_string()),
        ]
        .into();
        assert!(validate_provider_fields("codex", &values).is_err());
    }
}
