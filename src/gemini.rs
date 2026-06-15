use crate::errors::{AcsError, ConfigError};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{expand_path, Provider};

pub fn env_path(home: &str) -> PathBuf {
    PathBuf::from(expand_path(home)).join(".env")
}

pub fn parse_env(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }
    map
}

pub fn serialize_env(map: &HashMap<String, String>) -> String {
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();
    keys.iter().map(|k| format!("{k}={}\n", map[*k])).collect()
}

pub fn read_env(home: &str) -> Result<HashMap<String, String>, AcsError> {
    let path = env_path(home);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| ConfigError::load(&path, e))?;
    Ok(parse_env(&content))
}

pub fn write_env(home: &str, map: &HashMap<String, String>) -> Result<(), AcsError> {
    let path = env_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConfigError::dir_create(parent, e))?;
    }
    let content = serialize_env(map);
    std::fs::write(&path, content)
        .map_err(|e| ConfigError::save(&path, e))?;
    Ok(())
}

pub fn apply_provider(home: &str, provider: &Provider) -> Result<(), AcsError> {
    let mut env_map = read_env(home)?;
    env_map.remove("GEMINI_API_KEY");
    env_map.remove("GEMINI_MODEL");
    env_map.remove("GOOGLE_GEMINI_BASE_URL");

    env_map.insert(
        "GOOGLE_GEMINI_BASE_URL".to_string(),
        provider.base_url().to_string(),
    );
    for (key, val) in &provider.fields {
        if key == "GOOGLE_GEMINI_BASE_URL" {
            continue;
        }
        if !val.is_empty() {
            env_map.insert(key.clone(), val.clone());
        }
    }
    write_env(home, &env_map)?;

    let sp = crate::config::settings_path(home);
    let mut settings = crate::config::read_settings(home)?;
    let obj = settings
        .as_object_mut()
        .ok_or_else(|| ConfigError::parse(&sp, "settings.json is not a JSON object"))?;
    let security = obj
        .entry("security")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let security_obj = security
        .as_object_mut()
        .ok_or_else(|| ConfigError::parse(&sp, "settings.json security is not a JSON object"))?;
    let auth = security_obj
        .entry("auth")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let auth_obj = auth
        .as_object_mut()
        .ok_or_else(|| ConfigError::parse(&sp, "settings.json security.auth is not a JSON object"))?;
    auth_obj.insert(
        "selectedType".to_string(),
        serde_json::Value::String("gemini-api-key".to_string()),
    );

    crate::config::write_settings(home, &settings)
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
        let dir = env::temp_dir().join(format!("acp_gemini_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_provider(fields: std::collections::HashMap<String, String>) -> Provider {
        Provider { fields, fallback_urls: vec![] }
    }

    #[test]
    fn test_parse_env_basic() {
        let content = "# comment\nGOOGLE_GEMINI_BASE_URL=https://example.com\nGEMINI_API_KEY=sk-test\nGEMINI_MODEL=gemini-2.5-pro\n";
        let map = parse_env(content);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("GOOGLE_GEMINI_BASE_URL").unwrap(), "https://example.com");
        assert_eq!(map.get("GEMINI_API_KEY").unwrap(), "sk-test");
        assert_eq!(map.get("GEMINI_MODEL").unwrap(), "gemini-2.5-pro");
    }

    #[test]
    fn test_parse_env_skips_invalid() {
        let content = "VALID=ok\nINVALID LINE\nKEY_WITH-DASH=value\n";
        let map = parse_env(content);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("VALID").unwrap(), "ok");
    }

    #[test]
    fn test_serialize_env_sorted() {
        let mut map = HashMap::new();
        map.insert("B_KEY".to_string(), "b".to_string());
        map.insert("A_KEY".to_string(), "a".to_string());
        let output = serialize_env(&map);
        assert!(output.starts_with("A_KEY"));
        assert!(output.contains("B_KEY=b"));
    }

    #[test]
    fn test_read_write_env_roundtrip() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut map = HashMap::new();
        map.insert("GEMINI_API_KEY".to_string(), "sk-test".to_string());
        map.insert("GEMINI_MODEL".to_string(), "gemini-2.5-pro".to_string());
        write_env("~/.gemini", &map).unwrap();

        let loaded = read_env("~/.gemini").unwrap();
        assert_eq!(loaded.get("GEMINI_API_KEY").unwrap(), "sk-test");
        assert_eq!(loaded.get("GEMINI_MODEL").unwrap(), "gemini-2.5-pro");
    }

    #[test]
    fn test_read_env_missing_returns_empty() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let map = read_env("~/.gemini").unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_read_write_settings_roundtrip() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let json = serde_json::json!({
            "security": { "auth": { "selectedType": "oauth-personal" } },
            "ui": { "theme": "dark" }
        });
        crate::config::write_settings("~/.gemini", &json).unwrap();

        let loaded = crate::config::read_settings("~/.gemini").unwrap();
        assert_eq!(loaded["security"]["auth"]["selectedType"], "oauth-personal");
        assert_eq!(loaded["ui"]["theme"], "dark");
    }

    #[test]
    fn test_read_settings_missing_returns_empty_object() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let value = crate::config::read_settings("~/.gemini").unwrap();
        assert!(value.is_object());
        assert!(value.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_apply_provider_writes_env_and_settings() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://api.example.com".to_string());
            f.insert("GEMINI_API_KEY".to_string(), "sk-key-123".to_string());
            f
        });
        apply_provider("~/.gemini", &provider).unwrap();

        let env_map = read_env("~/.gemini").unwrap();
        assert_eq!(env_map.get("GOOGLE_GEMINI_BASE_URL").unwrap(), "https://api.example.com");
        assert_eq!(env_map.get("GEMINI_API_KEY").unwrap(), "sk-key-123");

        let settings = crate::config::read_settings("~/.gemini").unwrap();
        assert_eq!(settings["security"]["auth"]["selectedType"], "gemini-api-key");
    }

    #[test]
    fn test_apply_provider_with_model() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://api.example.com".to_string());
            f.insert("GEMINI_API_KEY".to_string(), "sk-key".to_string());
            f.insert("GEMINI_MODEL".to_string(), "gemini-3-pro".to_string());
            f
        });
        apply_provider("~/.gemini", &provider).unwrap();

        let env_map = read_env("~/.gemini").unwrap();
        assert_eq!(env_map.get("GEMINI_MODEL").unwrap(), "gemini-3-pro");
    }

    #[test]
    fn test_apply_provider_preserves_existing_settings() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let existing = serde_json::json!({
            "ui": { "theme": "GitHub" },
            "general": { "vimMode": true }
        });
        crate::config::write_settings("~/.gemini", &existing).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://api.example.com".to_string());
            f.insert("GEMINI_API_KEY".to_string(), "sk-key".to_string());
            f
        });
        apply_provider("~/.gemini", &provider).unwrap();

        let settings = crate::config::read_settings("~/.gemini").unwrap();
        assert_eq!(settings["ui"]["theme"], "GitHub");
        assert_eq!(settings["general"]["vimMode"], true);
        assert_eq!(settings["security"]["auth"]["selectedType"], "gemini-api-key");
    }

    #[test]
    fn test_apply_provider_clears_stale_credentials() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".gemini");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let p1 = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://api.one.com".to_string());
            f.insert("GEMINI_API_KEY".to_string(), "g-key-secret".to_string());
            f
        });
        apply_provider("~/.gemini", &p1).unwrap();

        let p2 = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert("GOOGLE_GEMINI_BASE_URL".to_string(), "https://api.two.com".to_string());
            f
        });
        apply_provider("~/.gemini", &p2).unwrap();

        let env_map = read_env("~/.gemini").unwrap();
        assert_eq!(env_map.get("GOOGLE_GEMINI_BASE_URL").unwrap(), "https://api.two.com");
        assert!(env_map.get("GEMINI_API_KEY").is_none_or(|v| v.is_empty()));
    }
}
