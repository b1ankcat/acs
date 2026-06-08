use crate::errors::{AcsError, ConfigError, ProviderError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AcsConfig {
    #[serde(default)]
    pub claude: ToolConfig,
    #[serde(default)]
    pub codex: ToolConfig,
    #[serde(default)]
    pub gemini: ToolConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolConfig {
    #[serde(default)]
    pub home: String,
    #[serde(default = "default_active")]
    pub active: String,
    #[serde(default)]
    pub providers: HashMap<String, Provider>,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            home: String::new(),
            active: default_active(),
            providers: HashMap::new(),
        }
    }
}

fn default_active() -> String {
    "default".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Provider {
    #[serde(flatten)]
    pub fields: HashMap<String, String>,
}

impl Provider {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    pub fn base_url(&self) -> &str {
        self.fields
            .get("ANTHROPIC_BASE_URL")
            .or_else(|| self.fields.get("base_url"))
            .or_else(|| self.fields.get("GOOGLE_GEMINI_BASE_URL"))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn model(&self, tool_name: &str) -> Option<&str> {
        match tool_name {
            "claude" => self.fields.get("ANTHROPIC_MODEL").map(String::as_str),
            "codex" => self.fields.get("model").map(String::as_str),
            "gemini" => self.fields.get("GEMINI_MODEL").map(String::as_str),
            _ => None,
        }
    }
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable is not set");
    PathBuf::from(home).join(".config/acs/config.toml")
}

pub fn load_config() -> Result<AcsConfig, AcsError> {
    let path = config_path();
    let content = std::fs::read_to_string(&path)
        .map_err(|e| ConfigError::load(&path, e))?;
    let config: AcsConfig = toml::from_str(&content)
        .map_err(|e| ConfigError::parse(&path, e.to_string()))?;
    Ok(config)
}

#[cfg(unix)]
fn set_permissions(path: &std::path::Path, mode: u32) -> Result<(), AcsError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| ConfigError::permissions(path, e))?
        .permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms)
        .map_err(|e| ConfigError::permissions(path, e))?;
    Ok(())
}

pub fn save_config(config: &AcsConfig) -> Result<(), AcsError> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConfigError::dir_create(parent, e))?;
        #[cfg(unix)]
        set_permissions(parent, 0o700)?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| ConfigError::serialize(e.to_string()))?;
    std::fs::write(&path, content)
        .map_err(|e| ConfigError::save(&path, e))?;
    #[cfg(unix)]
    set_permissions(&path, 0o600)?;
    Ok(())
}

pub fn expand_path(path: &str) -> String {
    let home = std::env::var("HOME").expect("HOME environment variable is not set");
    if let Some(rest) = path.strip_prefix("~/") {
        home + "/" + rest
    } else if let Some(rest) = path.strip_prefix("$HOME/") {
        home + "/" + rest
    } else if path == "~" || path == "$HOME" {
        home
    } else {
        path.to_string()
    }
}

/// Path to `settings.json` inside a tool's home directory.
pub fn settings_path(home: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(expand_path(home)).join("settings.json")
}

/// Read `settings.json` as a JSON value; returns an empty object if the file doesn't exist.
pub fn read_settings(home: &str) -> Result<serde_json::Value, AcsError> {
    let path = settings_path(home);
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| ConfigError::load(&path, e))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| ConfigError::parse(&path, e.to_string()))?;
    Ok(value)
}

/// Write a JSON value to `settings.json`, creating parent directories as needed.
pub fn write_settings(home: &str, value: &serde_json::Value) -> Result<(), AcsError> {
    let path = settings_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::dir_create(parent, e))?;
    }
    let content =
        serde_json::to_string_pretty(value).map_err(|e| ConfigError::serialize(e.to_string()))?;
    std::fs::write(&path, content).map_err(|e| ConfigError::save(&path, e))?;
    Ok(())
}

pub fn auto_import_defaults(config: &mut AcsConfig) -> Result<(), AcsError> {
    let home = std::env::var("HOME").expect("HOME environment variable is not set");
    if home.is_empty() {
        return Ok(());
    }

    let home_path = std::path::PathBuf::from(&home);

    let claude_dir = home_path.join(".claude");
    if let Some(provider) = crate::import_::parse_claude_native(&claude_dir)? {
        config.claude.providers.insert(default_active(), provider);
        config.claude.active = default_active();
    }

    let codex_dir = home_path.join(".codex");
    if let Some(provider) = crate::import_::parse_codex_native(&codex_dir)? {
        config.codex.providers.insert(default_active(), provider);
        config.codex.active = default_active();
    }

    let gemini_dir = home_path.join(".gemini");
    if let Some(provider) = crate::import_::parse_gemini_native(&gemini_dir)? {
        config.gemini.providers.insert(default_active(), provider);
        config.gemini.active = default_active();
    }

    Ok(())
}

pub fn ensure_tool_defaults(config: &mut AcsConfig) {
    for (tool, default_home) in [
        (&mut config.claude, "~/.claude"),
        (&mut config.codex, "~/.codex"),
        (&mut config.gemini, "~/.gemini"),
    ] {
        if tool.home.is_empty() {
            tool.home = default_home.to_string();
        }
        if tool.active.is_empty() {
            tool.active = default_active();
        }
    }
}

pub fn get_active_provider(tool: &ToolConfig) -> Option<&Provider> {
    tool.providers.get(&tool.active)
}

/// Reject provider names that are empty, contain path separators, traversal sequences, or control characters.
pub fn validate_provider_name(name: &str) -> Result<(), ProviderError> {
    if name.is_empty() {
        return Err(ProviderError::InvalidName(name.to_string()));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(ProviderError::InvalidName(name.to_string()));
    }
    if name.bytes().any(|b| b == 0 || b == b'\n' || b == b'\r') {
        return Err(ProviderError::InvalidName(name.to_string()));
    }
    Ok(())
}

impl AcsConfig {
    pub fn get_tool(&self, tool_name: &str) -> &ToolConfig {
        match tool_name {
            "claude" => &self.claude,
            "codex" => &self.codex,
            "gemini" => &self.gemini,
            _ => unreachable!(),
        }
    }

    pub fn get_tool_mut(&mut self, tool_name: &str) -> &mut ToolConfig {
        match tool_name {
            "claude" => &mut self.claude,
            "codex" => &mut self.codex,
            "gemini" => &mut self.gemini,
            _ => unreachable!(),
        }
    }
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

    fn setup_temp_home() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = env::temp_dir().join(format!("acp_config_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".config")).unwrap();
        env::set_current_dir(&dir).unwrap();
        dir
    }

    fn make_provider(fields: HashMap<String, String>) -> Provider {
        Provider { fields }
    }

    #[test]
    fn test_expand_path_tilde() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let result = expand_path("~/projects/myapp");
        assert!(result.starts_with(dir.to_str().unwrap()));
        assert!(result.ends_with("/projects/myapp"));
    }

    #[test]
    fn test_expand_path_dollar_home() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let result = expand_path("$HOME/projects/myapp");
        assert!(result.starts_with(dir.to_str().unwrap()));
        assert!(result.ends_with("/projects/myapp"));
    }

    #[test]
    #[should_panic(expected = "HOME environment variable is not set")]
    fn test_expand_path_no_home_var() {
        let _guard = home_lock();
        env::remove_var("HOME");
        expand_path("~/test");
    }

    #[test]
    fn test_config_path_contains_correct_suffix() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let path = config_path();
        let s = path.to_str().unwrap();
        assert!(s.ends_with(".config/acs/config.toml"));
    }

    #[test]
    fn test_load_config_missing_file_returns_error() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let result = load_config();
        assert!(result.is_err());
        // The OS error "No such file or directory" is propagated directly
        let err = result.unwrap_err();
        assert!(matches!(err, AcsError::Config(ConfigError::Load { .. })));
    }

    #[test]
    fn test_save_load_roundtrip_full_config() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = AcsConfig::default();
        cfg.claude.home = "~/.claude".to_string();
        cfg.claude.active = "anthropic-prod".to_string();
        cfg.claude.providers.insert(
            "anthropic-prod".to_string(),
            make_provider({
                let mut f = HashMap::new();
                f.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    "https://api.anthropic.com".to_string(),
                );
                f.insert(
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                    "sk-ant-secret".to_string(),
                );
                f.insert(
                    "ANTHROPIC_MODEL".to_string(),
                    "claude-sonnet-4-6".to_string(),
                );
                f
            }),
        );
        cfg.codex.home = "~/.codex".to_string();
        cfg.codex.active = "openai-prod".to_string();
        cfg.codex.providers.insert(
            "openai-prod".to_string(),
            make_provider({
                let mut f = HashMap::new();
                f.insert(
                    "base_url".to_string(),
                    "https://api.openai.com/v1".to_string(),
                );
                f.insert("openai_api_key".to_string(), "sk-openai-key".to_string());
                f.insert("model".to_string(), "gpt-5.5".to_string());
                f
            }),
        );
        save_config(&cfg).unwrap();

        let loaded = load_config().unwrap();
        assert_eq!(loaded.claude.home, "~/.claude");
        assert_eq!(loaded.claude.active, "anthropic-prod");
        assert_eq!(loaded.claude.providers.len(), 1);
        let p = &loaded.claude.providers["anthropic-prod"];
        assert_eq!(p.base_url(), "https://api.anthropic.com");
        assert_eq!(p.get("ANTHROPIC_AUTH_TOKEN"), Some("sk-ant-secret"));
        assert_eq!(p.get("openai_api_key"), None);
        assert_eq!(p.model("claude"), Some("claude-sonnet-4-6"));

        assert_eq!(loaded.codex.home, "~/.codex");
        assert_eq!(loaded.codex.active, "openai-prod");
        assert_eq!(loaded.codex.providers.len(), 1);
        let cp = &loaded.codex.providers["openai-prod"];
        assert_eq!(cp.get("openai_api_key"), Some("sk-openai-key"));
        assert_eq!(cp.model("codex"), Some("gpt-5.5"));
    }

    #[test]
    fn test_save_load_empty_config() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let cfg = AcsConfig::default();
        save_config(&cfg).unwrap();
        let loaded = load_config().unwrap();
        assert!(loaded.claude.providers.is_empty());
        assert!(loaded.codex.providers.is_empty());
    }

    #[test]
    fn test_provider_serialization_claude_fields_present() {
        let provider = make_provider({
            let mut f = HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://api.anthropic.com".to_string(),
            );
            f.insert(
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                "sk-ant-token-123".to_string(),
            );
            f.insert(
                "ANTHROPIC_MODEL".to_string(),
                "claude-sonnet-4-6".to_string(),
            );
            f
        });
        let toml_str = toml::to_string_pretty(&provider).unwrap();
        assert!(toml_str.contains("https://api.anthropic.com"));
        assert!(toml_str.contains("sk-ant-token-123"));
        assert!(toml_str.contains("ANTHROPIC_AUTH_TOKEN"));
        assert!(toml_str.contains("claude-sonnet-4-6"));
        // Provider no longer has a name field
        assert!(!toml_str.contains("name ="));
    }

    #[test]
    fn test_provider_serialization_codex_omits_auth_token() {
        let provider = make_provider({
            let mut f = HashMap::new();
            f.insert(
                "base_url".to_string(),
                "https://api.openai.com/v1".to_string(),
            );
            f.insert("openai_api_key".to_string(), "sk-openai-secret".to_string());
            f
        });
        let toml_str = toml::to_string_pretty(&provider).unwrap();
        assert!(toml_str.contains("openai_api_key"));
        assert!(toml_str.contains("sk-openai-secret"));
        assert!(!toml_str.contains("ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn test_provider_deserialization_preserves_fields() {
        let toml_str = r#"
ANTHROPIC_BASE_URL = "https://example.com"
ANTHROPIC_AUTH_TOKEN = "tok"
openai_api_key = "key"
model = "m1"
"#;
        let provider: Provider = toml::from_str(toml_str).unwrap();
        assert_eq!(provider.base_url(), "https://example.com");
        assert_eq!(provider.get("ANTHROPIC_AUTH_TOKEN"), Some("tok"));
        assert_eq!(provider.get("openai_api_key"), Some("key"));
        assert_eq!(provider.get("model"), Some("m1"));
    }

    #[test]
    fn test_provider_deserialization_omits_optional_fields() {
        let toml_str = r#"
base_url = "https://example.com"
"#;
        let provider: Provider = toml::from_str(toml_str).unwrap();
        assert!(provider.get("ANTHROPIC_AUTH_TOKEN").is_none());
        assert!(provider.get("openai_api_key").is_none());
        assert!(provider.get("model").is_none());
    }

    #[test]
    fn test_get_active_provider_returns_active() {
        let mut tool = ToolConfig {
            home: String::new(),
            active: "target".to_string(),
            providers: HashMap::new(),
        };
        tool.providers.insert(
            "other".to_string(),
            make_provider({
                let mut f = HashMap::new();
                f.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    "https://other.example.com".to_string(),
                );
                f
            }),
        );
        tool.providers.insert(
            "target".to_string(),
            make_provider({
                let mut f = HashMap::new();
                f.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    "https://target.example.com".to_string(),
                );
                f
            }),
        );
        let active = get_active_provider(&tool).unwrap();
        assert_eq!(active.base_url(), "https://target.example.com");
    }

    #[test]
    fn test_get_active_provider_none_when_no_providers() {
        let tool = ToolConfig {
            home: String::new(),
            active: "anything".to_string(),
            providers: HashMap::new(),
        };
        assert!(get_active_provider(&tool).is_none());
    }

    #[test]
    fn test_get_active_provider_none_when_no_match() {
        let mut tool = ToolConfig {
            home: String::new(),
            active: "missing".to_string(),
            providers: HashMap::new(),
        };
        tool.providers.insert(
            "present".to_string(),
            make_provider({
                let mut f = HashMap::new();
                f.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    "https://example.com".to_string(),
                );
                f
            }),
        );
        assert!(get_active_provider(&tool).is_none());
    }

    #[test]
    fn test_ensure_tool_defaults_fills_all_empty_fields() {
        let mut cfg = AcsConfig::default();
        cfg.claude.home.clear();
        cfg.claude.active.clear();
        cfg.codex.home.clear();
        cfg.codex.active.clear();
        ensure_tool_defaults(&mut cfg);
        assert_eq!(cfg.claude.home, "~/.claude");
        assert_eq!(cfg.claude.active, "default");
        assert_eq!(cfg.codex.home, "~/.codex");
        assert_eq!(cfg.codex.active, "default");
    }

    #[test]
    fn test_ensure_tool_defaults_preserves_custom_values() {
        let mut cfg = AcsConfig {
            claude: ToolConfig {
                home: "/custom/claude".to_string(),
                active: "custom-active".to_string(),
                providers: HashMap::new(),
            },
            codex: ToolConfig {
                home: "/custom/codex".to_string(),
                active: "custom-codex".to_string(),
                providers: HashMap::new(),
            },
            gemini: ToolConfig::default(),
        };
        ensure_tool_defaults(&mut cfg);
        assert_eq!(cfg.claude.home, "/custom/claude");
        assert_eq!(cfg.claude.active, "custom-active");
        assert_eq!(cfg.codex.home, "/custom/codex");
        assert_eq!(cfg.codex.active, "custom-codex");
    }

    #[test]
    fn test_tool_config_default_values() {
        let tc = ToolConfig::default();
        assert!(tc.home.is_empty());
        assert_eq!(tc.active, "default");
        assert!(tc.providers.is_empty());
    }

    #[test]
    fn test_acp_config_default_values() {
        let cfg = AcsConfig::default();
        assert_eq!(cfg.claude.active, "default");
        assert_eq!(cfg.codex.active, "default");
        assert!(cfg.claude.providers.is_empty());
        assert!(cfg.codex.providers.is_empty());
    }

    #[test]
    fn test_provider_model_claude() {
        let p = make_provider({
            let mut f = HashMap::new();
            f.insert("ANTHROPIC_MODEL".to_string(), "claude-opus-4-7".to_string());
            f
        });
        assert_eq!(p.model("claude"), Some("claude-opus-4-7"));
        assert_eq!(p.model("codex"), None);
        assert_eq!(p.model("gemini"), None);
    }

    #[test]
    fn test_provider_model_codex() {
        let p = make_provider({
            let mut f = HashMap::new();
            f.insert("model".to_string(), "gpt-5.5".to_string());
            f
        });
        assert_eq!(p.model("codex"), Some("gpt-5.5"));
        assert_eq!(p.model("claude"), None);
    }

    #[test]
    fn test_provider_model_gemini() {
        let p = make_provider({
            let mut f = HashMap::new();
            f.insert("GEMINI_MODEL".to_string(), "gemini-2.5-pro".to_string());
            f
        });
        assert_eq!(p.model("gemini"), Some("gemini-2.5-pro"));
        assert_eq!(p.model("claude"), None);
    }

    #[test]
    fn test_validate_provider_name_valid() {
        assert!(validate_provider_name("my-provider").is_ok());
        assert!(validate_provider_name("default").is_ok());
        assert!(validate_provider_name("openai_prod").is_ok());
    }

    #[test]
    fn test_validate_provider_name_rejects_invalid() {
        assert!(validate_provider_name("").is_err());
        assert!(validate_provider_name("path/traversal").is_err());
        assert!(validate_provider_name("back\\slash").is_err());
        assert!(validate_provider_name("dot..dot").is_err());
    }

    #[test]
    fn test_auto_import_from_existing_claude_config() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let claude_dir = dir.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                "ANTHROPIC_AUTH_TOKEN": "sk-ant-import",
                "ANTHROPIC_MODEL": "claude-sonnet-4-6"
            }
        });
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let mut cfg = AcsConfig::default();
        auto_import_defaults(&mut cfg).unwrap();

        assert_eq!(cfg.claude.providers.len(), 1);
        let p = &cfg.claude.providers["default"];
        assert_eq!(p.base_url(), "https://api.anthropic.com");
        assert_eq!(p.get("ANTHROPIC_AUTH_TOKEN"), Some("sk-ant-import"));
        assert_eq!(p.model("claude"), Some("claude-sonnet-4-6"));
        assert_eq!(cfg.claude.active, "default");
        assert!(cfg.codex.providers.is_empty());
        assert!(cfg.gemini.providers.is_empty());
    }

    #[test]
    fn test_auto_import_always_runs_regardless_of_config_file() {
        // auto_import_defaults no longer checks whether config exists —
        // that policy lives in load_config_with_defaults. Verify import runs.
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let cfg = AcsConfig::default();
        save_config(&cfg).unwrap();

        let claude_dir = dir.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("settings.json"),
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://import.example.com"}}"#,
        )
        .unwrap();

        let mut cfg = AcsConfig::default();
        auto_import_defaults(&mut cfg).unwrap();

        // The config file exists but auto_import_defaults imports anyway;
        // load_config_with_defaults is responsible for the skip-on-exists policy.
        assert_eq!(cfg.claude.providers.len(), 1);
        assert_eq!(
            cfg.claude.providers["default"].base_url(),
            "https://import.example.com"
        );
    }

    #[test]
    fn test_provider_base_url_priority() {
        // ANTHROPIC_BASE_URL takes priority over base_url over GOOGLE_GEMINI_BASE_URL
        let p = make_provider({
            let mut f = HashMap::new();
            f.insert("ANTHROPIC_BASE_URL".to_string(), "https://claude.example.com".to_string());
            f.insert("base_url".to_string(), "https://codex.example.com".to_string());
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://gemini.example.com".to_string());
            f
        });
        assert_eq!(p.base_url(), "https://claude.example.com");

        let p2 = make_provider({
            let mut f = HashMap::new();
            f.insert("base_url".to_string(), "https://codex.example.com".to_string());
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://gemini.example.com".to_string());
            f
        });
        assert_eq!(p2.base_url(), "https://codex.example.com");

        let p3 = make_provider({
            let mut f = HashMap::new();
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://gemini.example.com".to_string());
            f
        });
        assert_eq!(p3.base_url(), "https://gemini.example.com");

        let p4 = make_provider(HashMap::new());
        assert_eq!(p4.base_url(), "");
    }

    #[test]
    fn test_auto_import_from_existing_codex_config() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let codex_dir = dir.join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let config = r#"
model_provider = "custom"
base_url = "https://custom.openai.com"
model = "gpt-5.5"
requires_openai_auth = true

[model_providers.custom]
name = "custom"
base_url = "https://custom.openai.com"
requires_openai_auth = true
wire_api = "responses"
"#;
        fs::write(codex_dir.join("config.toml"), config).unwrap();
        fs::write(
            codex_dir.join("auth.json"),
            r#"{"OPENAI_API_KEY": "sk-codex-import"}"#,
        )
        .unwrap();

        let mut cfg = AcsConfig::default();
        auto_import_defaults(&mut cfg).unwrap();

        assert_eq!(cfg.codex.providers.len(), 1);
        let p = &cfg.codex.providers["default"];
        assert_eq!(p.base_url(), "https://custom.openai.com");
        assert_eq!(p.get("openai_api_key"), Some("sk-codex-import"));
        assert_eq!(p.model("codex"), Some("gpt-5.5"));
        assert_eq!(cfg.codex.active, "default");
    }

    #[test]
    fn test_auto_import_from_existing_gemini_config() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let gemini_dir = dir.join(".gemini");
        fs::create_dir_all(&gemini_dir).unwrap();
        fs::write(
            gemini_dir.join(".env"),
            "GOOGLE_GEMINI_BASE_URL=https://gemini.googleapis.com\nGEMINI_API_KEY=g-import-key\nGEMINI_MODEL=gemini-2.5-pro\n",
        )
        .unwrap();

        let mut cfg = AcsConfig::default();
        auto_import_defaults(&mut cfg).unwrap();

        assert_eq!(cfg.gemini.providers.len(), 1);
        let p = &cfg.gemini.providers["default"];
        assert_eq!(p.base_url(), "https://gemini.googleapis.com");
        assert_eq!(p.get("GEMINI_API_KEY"), Some("g-import-key"));
        assert_eq!(p.model("gemini"), Some("gemini-2.5-pro"));
        assert_eq!(cfg.gemini.active, "default");
    }

    #[test]
    fn test_expand_path_exact_tilde() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        assert_eq!(expand_path("~"), dir.to_str().unwrap());
    }

    #[test]
    fn test_expand_path_plain() {
        let _guard = home_lock();
        env::set_var("HOME", "/home/user");
        assert_eq!(expand_path("/absolute/path"), "/absolute/path");
    }

    #[test]
    fn test_expand_path_exact_dollar_home() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        assert_eq!(expand_path("$HOME"), dir.to_str().unwrap());
    }

    #[test]
    fn test_expand_path_dollar_home_prefix() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let result = expand_path("$HOME/projects/myapp");
        assert!(result.starts_with(dir.to_str().unwrap()));
        assert!(result.ends_with("/projects/myapp"));
    }

    #[test]
    fn test_provider_model_unknown_tool() {
        let p = make_provider(HashMap::new());
        assert_eq!(p.model("bogus"), None);
    }

    #[test]
    #[should_panic]
    fn test_get_tool_mut_panics_on_unknown() {
        let mut cfg = AcsConfig::default();
        let _ = cfg.get_tool_mut("bogus");
    }

    #[test]
    #[should_panic]
    fn test_get_tool_panics_on_unknown() {
        let cfg = AcsConfig::default();
        let _ = cfg.get_tool("bogus");
    }

    #[test]
    fn test_save_config_sets_permissions() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let cfg = AcsConfig::default();
        save_config(&cfg).unwrap();

        let path = config_path();
        let meta = std::fs::metadata(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    #[should_panic(expected = "HOME environment variable is not set")]
    fn test_auto_import_no_home() {
        let _guard = home_lock();
        env::remove_var("HOME");
        let mut cfg = AcsConfig::default();
        let _ = auto_import_defaults(&mut cfg);
    }

    #[test]
    fn test_auto_import_empty_home() {
        let _guard = home_lock();
        env::set_var("HOME", "");
        let mut cfg = AcsConfig::default();
        let result = auto_import_defaults(&mut cfg);
        assert!(result.is_ok());
        assert!(cfg.claude.providers.is_empty());
    }
}
