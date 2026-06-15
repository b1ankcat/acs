use crate::errors::{AcsError, ImportError};
use std::path::Path;

use crate::config::{self, Provider};

/// Statistics from an import operation.
pub struct ImportStats {
    pub imported: usize,
    pub skipped: usize,
}

/// Import providers from a TOML config file into the current config.
/// Skips providers whose name already exists, unless `force` is true.
pub fn import_from_toml(path: &str, cfg: &mut config::AcsConfig, force: bool) -> Result<ImportStats, AcsError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ImportError::read(path, e))?;
    let imported: config::AcsConfig = toml::from_str(&content)
        .map_err(|e| ImportError::parse(path, e.to_string()))?;

    let mut stats = ImportStats { imported: 0, skipped: 0 };

    for (target, source) in [
        (&mut cfg.claude, &imported.claude),
        (&mut cfg.codex, &imported.codex),
        (&mut cfg.gemini, &imported.gemini),
    ] {
        for (name, provider) in &source.providers {
            if config::validate_provider_name(name).is_err() {
                return Err(ImportError::parse(path, format!("invalid provider name: {:?}", name)).into());
            }
            if target.providers.contains_key(name) && !force {
                stats.skipped += 1;
                continue;
            }
            target.providers.insert(name.clone(), provider.clone());
            stats.imported += 1;
        }
    }

    Ok(stats)
}

pub(crate) fn parse_claude_native(dir: &Path) -> Result<Option<Provider>, AcsError> {
    let settings_path = dir.join("settings.json");
    if !settings_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| ImportError::read(&settings_path, e))?;
    let json: serde_json::Value =
        serde_json::from_str(&content)
            .map_err(|e| ImportError::parse(&settings_path, e.to_string()))?;

    let env = json.get("env").and_then(|v| v.as_object());
    let mut fields = std::collections::HashMap::new();

    for key in &[
        "ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ] {
        if let Some(v) = env.and_then(|e| e.get(*key).and_then(|v| v.as_str())) {
            if !v.is_empty() {
                fields.insert(key.to_string(), v.to_string());
            }
        }
    }

    // A provider without a base URL is unusable; treat as nothing to import.
    Ok(if fields.contains_key("ANTHROPIC_BASE_URL") { Some(Provider { fields, ..Default::default() }) } else { None })
}

pub(crate) fn parse_codex_native(dir: &Path) -> Result<Option<Provider>, AcsError> {
    let auth_path = dir.join("auth.json");
    let config_path = dir.join("config.toml");

    let mut fields = std::collections::HashMap::new();

    if auth_path.exists() {
        let content = std::fs::read_to_string(&auth_path)
            .map_err(|e| ImportError::read(&auth_path, e))?;
        let json: serde_json::Value =
            serde_json::from_str(&content)
                .map_err(|e| ImportError::parse(&auth_path, e.to_string()))?;
        if let Some(key) = json.get("OPENAI_API_KEY").and_then(|v| v.as_str()) {
            fields.insert("openai_api_key".to_string(), key.to_string());
        }
    }

    if !config_path.exists() {
        // No config.toml — if we read an API key from auth.json, we still can't
        // produce a usable provider without a base_url. Return None.
        return Ok(None);
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| ImportError::read(&config_path, e))?;
    let toml_val: toml::Value = toml::from_str(&content)
        .map_err(|e| ImportError::parse(&config_path, e.to_string()))?;

    if let Some(mp_table) = toml_val.get("model_providers").and_then(|v| v.as_table()) {
        // Use the active provider name to look up the correct entry, not iteration order.
        let active_name = toml_val.get("model_provider").and_then(|v| v.as_str()).unwrap_or("");
        let active_entry = mp_table.get(active_name).or_else(|| mp_table.values().next());
        if let Some(t) = active_entry.and_then(|v| v.as_table()) {
            for key in &["base_url", "wire_api", "name"] {
                if let Some(v) = t.get(*key).and_then(|v| v.as_str()) {
                    if !v.is_empty() {
                        fields.insert(key.to_string(), v.to_string());
                    }
                }
            }
            if let Some(v) = t.get("requires_openai_auth").and_then(|v| v.as_bool()) {
                fields.insert("requires_openai_auth".to_string(), v.to_string());
            }
        }
    }

    for key in &["model", "model_provider", "model_reasoning_effort"] {
        if let Some(v) = toml_val.get(*key).and_then(|v| v.as_str()) {
            if !v.is_empty() {
                fields.insert(key.to_string(), v.to_string());
            }
        }
    }
    if let Some(v) = toml_val
        .get("disable_response_storage")
        .and_then(|v| v.as_bool())
    {
        fields.insert("disable_response_storage".to_string(), v.to_string());
    }
    if let Some(v) = toml_val
        .get("requires_openai_auth")
        .and_then(|v| v.as_bool())
    {
        fields.insert("requires_openai_auth".to_string(), v.to_string());
    }

    // A provider without base_url is unusable; treat as nothing to import.
    Ok(if fields.contains_key("base_url") { Some(Provider { fields, ..Default::default() }) } else { None })
}

pub(crate) fn parse_gemini_native(dir: &Path) -> Result<Option<Provider>, AcsError> {
    let env_path = dir.join(".env");
    if !env_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&env_path)
        .map_err(|e| ImportError::read(&env_path, e))?;
    let env_map = crate::gemini::parse_env(&content);

    let mut fields = std::collections::HashMap::new();
    for key in &["GOOGLE_GEMINI_BASE_URL", "GEMINI_API_KEY", "GEMINI_MODEL"] {
        if let Some(val) = env_map.get(*key) {
            if !val.is_empty() {
                fields.insert(key.to_string(), val.clone());
            }
        }
    }

    // A provider without base URL is unusable; treat as nothing to import.
    Ok(if fields.contains_key("GOOGLE_GEMINI_BASE_URL") { Some(Provider { fields, fallback_urls: vec![] }) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn home_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::HOME_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn setup_temp_dir() -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = env::temp_dir().join(format!("acp_import_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_claude_provider(url: &str, token: &str) -> Provider {
        let mut f = std::collections::HashMap::new();
        f.insert("ANTHROPIC_BASE_URL".to_string(), url.to_string());
        f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), token.to_string());
        Provider { fields: f, fallback_urls: vec![] }
    }

    // ── TOML import tests ──

    #[test]
    fn test_import_from_toml_adds_providers() {
        let dir = setup_temp_dir();
        let _guard = home_lock();

        let toml_path = dir.join("import.toml");
        let toml_content = r#"
[claude.providers.my-claude]
ANTHROPIC_BASE_URL = "https://api.anthropic.com"
ANTHROPIC_AUTH_TOKEN = "sk-ant-123"
ANTHROPIC_MODEL = "claude-sonnet-4-6"

[codex.providers.my-codex]
base_url = "https://api.openai.com/v1"
openai_api_key = "sk-openai-456"
model = "gpt-5.5"
"#;
        fs::write(&toml_path, toml_content).unwrap();

        let mut cfg = config::AcsConfig::default();
        let stats = import_from_toml(toml_path.to_str().unwrap(), &mut cfg, false).unwrap();
        assert_eq!(stats.imported, 2);
        assert_eq!(stats.skipped, 0);
        assert_eq!(cfg.claude.providers.len(), 1);
        assert_eq!(cfg.claude.providers["my-claude"].base_url(), "https://api.anthropic.com");
        assert_eq!(cfg.codex.providers.len(), 1);
        assert_eq!(cfg.codex.providers["my-codex"].get("openai_api_key"), Some("sk-openai-456"));
    }

    #[test]
    fn test_import_from_toml_skips_existing() {
        let dir = setup_temp_dir();
        let _guard = home_lock();

        let toml_path = dir.join("import.toml");
        fs::write(&toml_path, r#"
[claude.providers.my-claude]
ANTHROPIC_BASE_URL = "https://api.new.com"
ANTHROPIC_AUTH_TOKEN = "new-token"
"#).unwrap();

        let mut cfg = config::AcsConfig::default();
        cfg.claude.providers.insert("my-claude".to_string(), make_claude_provider("https://old.com", "old-token"));

        let stats = import_from_toml(toml_path.to_str().unwrap(), &mut cfg, false).unwrap();
        assert_eq!(stats.imported, 0);
        assert_eq!(stats.skipped, 1);
        assert_eq!(cfg.claude.providers["my-claude"].base_url(), "https://old.com");
    }

    #[test]
    fn test_import_from_toml_force_overwrites() {
        let dir = setup_temp_dir();
        let _guard = home_lock();

        let toml_path = dir.join("import.toml");
        fs::write(&toml_path, r#"
[claude.providers.my-claude]
ANTHROPIC_BASE_URL = "https://api.new.com"
ANTHROPIC_AUTH_TOKEN = "new-token"
"#).unwrap();

        let mut cfg = config::AcsConfig::default();
        cfg.claude.providers.insert("my-claude".to_string(), make_claude_provider("https://old.com", "old-token"));

        let stats = import_from_toml(toml_path.to_str().unwrap(), &mut cfg, true).unwrap();
        assert_eq!(stats.imported, 1);
        assert_eq!(stats.skipped, 0);
        assert_eq!(cfg.claude.providers["my-claude"].base_url(), "https://api.new.com");
    }

    #[test]
    fn test_import_from_toml_bad_path() {
        let _guard = home_lock();
        let mut cfg = config::AcsConfig::default();
        let result = import_from_toml("/nonexistent/import.toml", &mut cfg, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_import_from_toml_invalid_format() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        let toml_path = dir.join("bad.toml");
        fs::write(&toml_path, "not valid toml {{{").unwrap();

        let mut cfg = config::AcsConfig::default();
        let result = import_from_toml(toml_path.to_str().unwrap(), &mut cfg, false);
        assert!(result.is_err());
    }

    // ── Native parse tests ──

    #[test]
    fn test_parse_claude_native_roundtrip() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        let settings = serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                "ANTHROPIC_AUTH_TOKEN": "sk-ant-123"
            }
        });
        fs::write(dir.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();

        let parsed = parse_claude_native(&dir).unwrap().unwrap();
        assert_eq!(parsed.base_url(), "https://api.anthropic.com");
        assert_eq!(parsed.get("ANTHROPIC_AUTH_TOKEN"), Some("sk-ant-123"));
    }

    #[test]
    fn test_parse_codex_native_roundtrip() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        fs::write(dir.join("auth.json"), r#"{"OPENAI_API_KEY": "sk-openai"}"#).unwrap();
        fs::write(dir.join("config.toml"), r#"
model_provider = "p"
model = "gpt-5.5"

[model_providers.p]
base_url = "https://api.openai.com/v1"
"#).unwrap();

        let parsed = parse_codex_native(&dir).unwrap().unwrap();
        assert_eq!(parsed.get("openai_api_key"), Some("sk-openai"));
        assert_eq!(parsed.get("model"), Some("gpt-5.5"));
        assert_eq!(parsed.get("base_url"), Some("https://api.openai.com/v1"));
    }

    #[test]
    fn test_parse_codex_native_missing_base_url_returns_none() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        fs::write(dir.join("auth.json"), r#"{"OPENAI_API_KEY": "sk-openai"}"#).unwrap();
        fs::write(dir.join("config.toml"), "model_provider = \"p\"\nmodel = \"gpt-5.5\"\n").unwrap();

        // No base_url in model_providers → unusable provider, return None
        assert!(parse_codex_native(&dir).unwrap().is_none());
    }

    #[test]
    fn test_parse_gemini_native_roundtrip() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        fs::write(dir.join(".env"), "GOOGLE_GEMINI_BASE_URL=https://api.example.com\nGEMINI_API_KEY=g-key-123\n").unwrap();

        let parsed = parse_gemini_native(&dir).unwrap().unwrap();
        assert_eq!(parsed.base_url(), "https://api.example.com");
        assert_eq!(parsed.get("GEMINI_API_KEY"), Some("g-key-123"));
    }

    #[test]
    fn test_parse_gemini_native_missing_base_url_returns_none() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        fs::write(dir.join(".env"), "GEMINI_API_KEY=g-key-only\n").unwrap();

        assert!(parse_gemini_native(&dir).unwrap().is_none());
    }

    #[test]
    fn test_parse_claude_native_with_haiku_sonnet_opus() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        let settings = serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.example.com",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5",
                "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-4-6",
                "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-4-7"
            }
        });
        fs::write(dir.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();

        let provider = parse_claude_native(&dir).unwrap().unwrap();
        assert_eq!(provider.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"), Some("claude-haiku-4-5"));
        assert_eq!(provider.get("ANTHROPIC_DEFAULT_SONNET_MODEL"), Some("claude-sonnet-4-6"));
        assert_eq!(provider.get("ANTHROPIC_DEFAULT_OPUS_MODEL"), Some("claude-opus-4-7"));
    }

    #[test]
    fn test_parse_claude_native_missing_dir() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        assert!(parse_claude_native(&dir).unwrap().is_none());
    }

    #[test]
    fn test_parse_codex_native_missing_config() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        assert!(parse_codex_native(&dir).unwrap().is_none());
    }

    #[test]
    fn test_parse_gemini_native_missing_dir() {
        let dir = setup_temp_dir();
        let _guard = home_lock();
        assert!(parse_gemini_native(&dir).unwrap().is_none());
    }
}
