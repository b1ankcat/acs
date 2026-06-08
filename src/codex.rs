use crate::clear::ClearTarget;
use crate::errors::{AcsError, ConfigError};
use std::path::PathBuf;
use toml::Value;

use crate::config::{expand_path, Provider};

const CLEAR_CONTENT_DIRS: &[&str] = &[
    "sessions",
    "shell_snapshots",
    "tmp",
    "archived_sessions",
    ".sandbox",
    ".tmp",
];

pub fn config_path(home: &str) -> PathBuf {
    PathBuf::from(expand_path(home)).join("config.toml")
}

pub fn read_config(home: &str) -> Result<Value, AcsError> {
    let path = config_path(home);
    if !path.exists() {
        return Ok(Value::Table(toml::Table::new()));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| ConfigError::load(&path, e))?;
    let value: Value =
        toml::from_str(&content).map_err(|e| ConfigError::parse(&path, e.to_string()))?;
    Ok(value)
}

pub fn write_config(home: &str, value: &Value) -> Result<(), AcsError> {
    let path = config_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::dir_create(parent, e))?;
    }
    let content =
        toml::to_string_pretty(value).map_err(|e| ConfigError::serialize(e.to_string()))?;
    std::fs::write(&path, content).map_err(|e| ConfigError::save(&path, e))?;
    Ok(())
}

pub fn apply_provider(home: &str, provider: &Provider) -> Result<(), AcsError> {
    let path = config_path(home);
    let mut value = read_config(home)?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| ConfigError::parse(&path, "config.toml is not a TOML table"))?;

    // Clear stale top-level fields. base_url, requires_openai_auth, openai_api_key
    // only live in [model_providers.name] and auth.json respectively.
    for key in &[
        "model",
        "model_provider",
        "disable_response_storage",
        "model_reasoning_effort",
        "base_url",
        "requires_openai_auth",
        "openai_api_key",
    ] {
        table.remove(*key);
    }

    // Top-level fields: model, model_provider, model_reasoning_effort, disable_response_storage
    let key_map: &[(&str, bool)] = &[
        ("model", false),
        ("model_provider", false),
        ("model_reasoning_effort", false),
        ("disable_response_storage", true),
    ];
    for &(toml_key, is_bool) in key_map {
        if let Some(val) = provider.fields.get(toml_key) {
            if !val.is_empty() {
                if is_bool {
                    table.insert(toml_key.to_string(), Value::Boolean(val == "true"));
                } else {
                    table.insert(toml_key.to_string(), Value::String(val.clone()));
                }
            }
        }
    }

    // [model_providers.<name>] section — base_url, requires_openai_auth, wire_api ONLY here
    let base_url = provider.get("base_url").ok_or_else(|| {
        ConfigError::parse(config_path(home), "provider missing required field: base_url")
    })?;
    let provider_name = provider
        .get("model_provider")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ConfigError::parse(config_path(home), "provider missing required field: model_provider")
        })?;
    let requires_auth = provider
        .get("requires_openai_auth")
        .map(|v| v == "true")
        .unwrap_or(true);
    let wire_api = provider.get("wire_api");

    let mp = table
        .entry("model_providers".to_string())
        .or_insert_with(|| Value::Table(toml::Table::new()));
    let mp_table = mp.as_table_mut()
        .ok_or_else(|| ConfigError::parse(config_path(home), "model_providers is not a TOML table"))?;
    let provider_entry = mp_table
        .entry(provider_name.to_string())
        .or_insert_with(|| Value::Table(toml::Table::new()));
    let pt = provider_entry.as_table_mut()
        .ok_or_else(|| ConfigError::parse(config_path(home), "model_providers entry is not a TOML table"))?;
    pt.insert("name".to_string(), Value::String(provider_name.to_string()));
    pt.insert("base_url".to_string(), Value::String(base_url.to_string()));
    pt.insert("requires_openai_auth".to_string(), Value::Boolean(requires_auth));
    if let Some(w) = wire_api {
        if !w.is_empty() {
            pt.insert("wire_api".to_string(), Value::String(w.to_string()));
        }
    }

    // Write auth.json — OPENAI_API_KEY lives here, never in config.toml
    if let Some(api_key) = provider.get("openai_api_key") {
        let auth_dir = PathBuf::from(expand_path(home));
        std::fs::create_dir_all(&auth_dir).map_err(|e| ConfigError::dir_create(&auth_dir, e))?;
        let auth = serde_json::json!({
            "OPENAI_API_KEY": api_key
        });
        let auth_path = auth_dir.join("auth.json");
        let content = serde_json::to_string_pretty(&auth)
            .map_err(|e| ConfigError::serialize(e.to_string()))?;
        std::fs::write(&auth_path, content).map_err(|e| ConfigError::save(&auth_path, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&auth_path)
                .map_err(|e| ConfigError::permissions(&auth_path, e))?
                .permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&auth_path, perms)
                .map_err(|e| ConfigError::permissions(&auth_path, e))?;
        }
    }

    write_config(home, &value)
}

pub fn clear_targets(home: &str) -> Vec<ClearTarget> {
    let tool_home = PathBuf::from(expand_path(home));
    let mut targets = vec![ClearTarget::file_or_dir(tool_home.join("history.jsonl"))];

    targets.extend(
        CLEAR_CONTENT_DIRS
            .iter()
            .map(|name| ClearTarget::dir_contents(tool_home.join(name))),
    );

    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clear;
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn home_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::HOME_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn setup_temp_home() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = env::temp_dir().join(format!("acp_codex_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_provider(fields: std::collections::HashMap<String, String>) -> Provider {
        Provider { fields }
    }

    #[test]
    fn test_read_config_missing_returns_empty_table() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let value = read_config("~/.codex").unwrap();
        assert!(value.is_table());
        assert!(value.as_table().unwrap().is_empty());
    }

    #[test]
    fn test_write_read_roundtrip_preserves_all_fields() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str(
            r#"
model_provider = "openai"
base_url = "https://api.openai.com/v1"
requires_openai_auth = true
openai_api_key = "sk-old-key"
model = "gpt-4"
"#,
        )
        .unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        assert_eq!(loaded["model_provider"].as_str().unwrap(), "openai");
        assert_eq!(
            loaded["base_url"].as_str().unwrap(),
            "https://api.openai.com/v1"
        );
        assert!(loaded["requires_openai_auth"].as_bool().unwrap());
        assert_eq!(loaded["openai_api_key"].as_str().unwrap(), "sk-old-key");
        assert_eq!(loaded["model"].as_str().unwrap(), "gpt-4");
    }

    #[test]
    fn test_apply_provider_updates_core_fields() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str("").unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "base_url".to_string(),
                "https://custom.api.com/v1".to_string(),
            );
            f.insert("openai_api_key".to_string(), "sk-custom-key".to_string());
            f.insert("model_provider".to_string(), "custom-provider".to_string());
            f.insert("requires_openai_auth".to_string(), "true".to_string());
            f
        });
        apply_provider("~/.codex", &provider).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        // Top-level fields
        assert_eq!(
            loaded["model_provider"].as_str().unwrap(),
            "custom-provider"
        );
        // base_url, requires_openai_auth only in [model_providers.custom-provider]
        assert!(loaded.get("base_url").is_none());
        assert!(loaded.get("requires_openai_auth").is_none());
        assert!(loaded.get("openai_api_key").is_none());
        let mp = &loaded["model_providers"]["custom-provider"];
        assert_eq!(
            mp["base_url"].as_str().unwrap(),
            "https://custom.api.com/v1"
        );
        assert!(mp["requires_openai_auth"].as_bool().unwrap());
        // auth.json has the API key
        let auth_content = fs::read_to_string(home_dir.join("auth.json")).unwrap();
        assert!(auth_content.contains("sk-custom-key"));
    }

    #[test]
    fn test_apply_provider_preserves_existing_model_providers_section() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str(
            r#"
[model_providers]
[model_providers.rightcode]
name = "rightcode"
base_url = "https://rightcode.example.com"
requires_openai_auth = true
"#,
        )
        .unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert("base_url".to_string(), "https://new.api.com/v1".to_string());
            f.insert("openai_api_key".to_string(), "sk-new".to_string());
            f.insert("model_provider".to_string(), "new-provider".to_string());
            f.insert("requires_openai_auth".to_string(), "true".to_string());
            f
        });
        apply_provider("~/.codex", &provider).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        // Top-level model_provider set
        assert_eq!(loaded["model_provider"].as_str().unwrap(), "new-provider");
        // No top-level base_url, requires_openai_auth
        assert!(loaded.get("base_url").is_none());
        // Existing model_providers section preserved
        let mp = &loaded["model_providers"];
        assert!(mp.get("rightcode").is_some());
        assert_eq!(mp["rightcode"]["name"].as_str().unwrap(), "rightcode");
        // New provider in model_providers section
        assert!(mp.get("new-provider").is_some());
        assert_eq!(
            mp["new-provider"]["base_url"].as_str().unwrap(),
            "https://new.api.com/v1"
        );
    }

    #[test]
    fn test_apply_provider_with_model_reasoning_effort() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str("").unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "base_url".to_string(),
                "https://api.example.com".to_string(),
            );
            f.insert("openai_api_key".to_string(), "sk-key".to_string());
            f.insert("model_provider".to_string(), "p".to_string());
            f.insert("model_reasoning_effort".to_string(), "high".to_string());
            f.insert("requires_openai_auth".to_string(), "true".to_string());
            f
        });
        apply_provider("~/.codex", &provider).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        assert_eq!(loaded["model_reasoning_effort"].as_str().unwrap(), "high");
    }

    #[test]
    fn test_apply_provider_with_disable_response_storage_true() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str("").unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "base_url".to_string(),
                "https://api.example.com".to_string(),
            );
            f.insert("openai_api_key".to_string(), "sk-key".to_string());
            f.insert("model_provider".to_string(), "p".to_string());
            f.insert("disable_response_storage".to_string(), "true".to_string());
            f.insert("requires_openai_auth".to_string(), "true".to_string());
            f
        });
        apply_provider("~/.codex", &provider).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        assert!(loaded["disable_response_storage"].as_bool().unwrap());
    }

    #[test]
    fn test_apply_provider_with_disable_response_storage_false() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str("").unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "base_url".to_string(),
                "https://api.example.com".to_string(),
            );
            f.insert("openai_api_key".to_string(), "sk-key".to_string());
            f.insert("model_provider".to_string(), "p".to_string());
            f.insert("disable_response_storage".to_string(), "false".to_string());
            f.insert("requires_openai_auth".to_string(), "true".to_string());
            f
        });
        apply_provider("~/.codex", &provider).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        assert!(!loaded["disable_response_storage"].as_bool().unwrap());
    }

    #[test]
    fn test_apply_provider_sets_model_from_provider() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let toml_val: toml::Value = toml::from_str("").unwrap();
        write_config("~/.codex", &toml_val).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "base_url".to_string(),
                "https://api.example.com".to_string(),
            );
            f.insert("openai_api_key".to_string(), "sk-key".to_string());
            f.insert("model".to_string(), "gpt-5.5".to_string());
            f.insert("model_provider".to_string(), "p".to_string());
            f.insert("requires_openai_auth".to_string(), "true".to_string());
            f
        });
        apply_provider("~/.codex", &provider).unwrap();

        let loaded = read_config("~/.codex").unwrap();
        assert_eq!(loaded["model"].as_str().unwrap(), "gpt-5.5");
    }

    #[test]
    fn test_config_path_expands_home() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let path = config_path("~/.codex");
        assert!(path.to_str().unwrap().starts_with(dir.to_str().unwrap()));
        assert!(path.to_str().unwrap().ends_with(".codex/config.toml"));
    }

    #[test]
    fn test_clear_state_removes_codex_sessions_and_cache() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".codex");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        fs::write(home_dir.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
        fs::write(home_dir.join("auth.json"), "{}").unwrap();
        fs::write(home_dir.join("history.jsonl"), "history").unwrap();
        fs::write(home_dir.join("keep.txt"), "keep").unwrap();

        for name in CLEAR_CONTENT_DIRS.iter().copied() {
            let target = home_dir.join(name);
            fs::create_dir_all(target.join("nested")).unwrap();
            fs::write(target.join("entry.txt"), "entry").unwrap();
            fs::write(target.join("nested").join("entry.txt"), "nested").unwrap();
        }

        let targets = clear_targets("~/.codex");
        let stats = clear::clear_targets(&targets).unwrap();

        assert_eq!(stats.removed, 13);
        assert!(home_dir.join("config.toml").exists());
        assert!(home_dir.join("auth.json").exists());
        assert!(!home_dir.join("history.jsonl").exists());
        assert!(home_dir.join("keep.txt").exists());
        for name in CLEAR_CONTENT_DIRS.iter().copied() {
            let target = home_dir.join(name);
            assert!(target.exists());
            assert!(fs::read_dir(target).unwrap().next().is_none());
        }
    }
}
