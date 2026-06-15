use crate::clear::ClearTarget;
use crate::errors::{AcsError, ConfigError};
use serde_json::Value;
use std::path::PathBuf;

use crate::config::{expand_path, Provider};

const CLEAR_CONTENT_DIRS: &[&str] = &[
    "file-history",
    "paste-cache",
    "projects",
    "session-env",
    "sessions",
    "tasks",
    "telemetry",
    "backups",
    "shell_snapshots",
];

pub fn apply_provider(home: &str, provider: &Provider) -> Result<(), AcsError> {
    let mut value = crate::config::read_settings(home)?;
    let sp = crate::config::settings_path(home);

    let map = value
        .as_object_mut()
        .ok_or_else(|| ConfigError::parse(&sp, "settings.json is not a JSON object"))?;

    let env = map
        .entry("env")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let env_map = env
        .as_object_mut()
        .ok_or_else(|| ConfigError::parse(&sp, "settings.json env is not a JSON object"))?;

    let claude_keys = [
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ];
    for key in &claude_keys {
        env_map.remove(*key);
    }

    env_map.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        Value::String(provider.base_url().to_string()),
    );

    for (key, val) in &provider.fields {
        if key == "ANTHROPIC_BASE_URL" {
            continue;
        }
        if !val.is_empty() {
            env_map.insert(key.clone(), Value::String(val.clone()));
        }
    }

    crate::config::write_settings(home, &value)
}

pub fn clear_targets(home: &str) -> Vec<ClearTarget> {
    let tool_home = PathBuf::from(expand_path(home));
    let claude_json = PathBuf::from(expand_path("~/.claude.json"));
    let mut targets = vec![
        ClearTarget::file_or_dir(claude_json),
        ClearTarget::file_or_dir(tool_home.join("history.jsonl")),
    ];

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
        let dir = env::temp_dir().join(format!("acp_claude_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_provider(fields: std::collections::HashMap<String, String>) -> Provider {
        Provider { fields, fallback_urls: vec![] }
    }

    #[test]
    fn test_read_settings_missing_returns_empty_object() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let settings_file = home_dir.join("settings.json");
        if settings_file.exists() {
            fs::remove_file(&settings_file).unwrap();
        }
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let value = crate::config::read_settings("~/.claude").unwrap();
        assert!(value.is_object());
        assert!(value.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_write_read_roundtrip_preserves_all_fields() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let json = serde_json::json!({
            "permissions": {
                "allow": ["Bash", "Read"]
            },
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                "ANTHROPIC_AUTH_TOKEN": "sk-ant-old"
            },
            "effortLevel": "high"
        });
        crate::config::write_settings("~/.claude", &json).unwrap();

        let loaded = crate::config::read_settings("~/.claude").unwrap();
        assert_eq!(loaded["permissions"]["allow"][0].as_str().unwrap(), "Bash");
        assert_eq!(
            loaded["env"]["ANTHROPIC_BASE_URL"].as_str().unwrap(),
            "https://api.anthropic.com"
        );
        assert_eq!(
            loaded["env"]["ANTHROPIC_AUTH_TOKEN"].as_str().unwrap(),
            "sk-ant-old"
        );
        assert_eq!(loaded["effortLevel"].as_str().unwrap(), "high");
    }

    #[test]
    fn test_apply_provider_updates_env_vars() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let json = serde_json::json!({
            "permissions": { "allow": ["Read"] }
        });
        crate::config::write_settings("~/.claude", &json).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://custom.api.com".to_string(),
            );
            f.insert(
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                "sk-custom-key".to_string(),
            );
            f
        });
        apply_provider("~/.claude", &provider).unwrap();

        let loaded = crate::config::read_settings("~/.claude").unwrap();
        assert_eq!(
            loaded["env"]["ANTHROPIC_BASE_URL"].as_str().unwrap(),
            "https://custom.api.com"
        );
        assert_eq!(
            loaded["env"]["ANTHROPIC_AUTH_TOKEN"].as_str().unwrap(),
            "sk-custom-key"
        );
    }

    #[test]
    fn test_apply_provider_preserves_non_env_fields() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let json = serde_json::json!({
            "permissions": {
                "allow": ["Bash", "Read", "Write"]
            },
            "effortLevel": "medium"
        });
        crate::config::write_settings("~/.claude", &json).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://test.api.com".to_string(),
            );
            f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-test".to_string());
            f
        });
        apply_provider("~/.claude", &provider).unwrap();

        let loaded = crate::config::read_settings("~/.claude").unwrap();
        assert_eq!(loaded["permissions"]["allow"][0].as_str().unwrap(), "Bash");
        assert_eq!(loaded["permissions"]["allow"][1].as_str().unwrap(), "Read");
        assert_eq!(loaded["permissions"]["allow"][2].as_str().unwrap(), "Write");
        assert_eq!(loaded["effortLevel"].as_str().unwrap(), "medium");
    }

    #[test]
    fn test_apply_provider_sets_model_from_provider() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let json = serde_json::json!({});
        crate::config::write_settings("~/.claude", &json).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://api.example.com".to_string(),
            );
            f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-key".to_string());
            f.insert("ANTHROPIC_MODEL".to_string(), "claude-opus-4-7".to_string());
            f
        });
        apply_provider("~/.claude", &provider).unwrap();

        let loaded = crate::config::read_settings("~/.claude").unwrap();
        assert_eq!(
            loaded["env"]["ANTHROPIC_MODEL"].as_str().unwrap(),
            "claude-opus-4-7"
        );
    }

    #[test]
    fn test_apply_provider_sets_haiku_sonnet_opus_models() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let json = serde_json::json!({});
        crate::config::write_settings("~/.claude", &json).unwrap();

        let provider = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://api.example.com".to_string(),
            );
            f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-key".to_string());
            f.insert(
                "ANTHROPIC_DEFAULT_HAIKU_MODEL".to_string(),
                "claude-haiku-4-5-20251001".to_string(),
            );
            f.insert(
                "ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(),
                "claude-sonnet-4-6".to_string(),
            );
            f.insert(
                "ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(),
                "claude-opus-4-7".to_string(),
            );
            f
        });
        apply_provider("~/.claude", &provider).unwrap();

        let loaded = crate::config::read_settings("~/.claude").unwrap();
        assert_eq!(
            loaded["env"]["ANTHROPIC_DEFAULT_HAIKU_MODEL"]
                .as_str()
                .unwrap(),
            "claude-haiku-4-5-20251001"
        );
        assert_eq!(
            loaded["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"]
                .as_str()
                .unwrap(),
            "claude-sonnet-4-6"
        );
        assert_eq!(
            loaded["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"]
                .as_str()
                .unwrap(),
            "claude-opus-4-7"
        );
    }

    #[test]
    fn test_settings_path_expands_home() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let path = crate::config::settings_path("~/.claude");
        assert!(path.to_str().unwrap().starts_with(dir.to_str().unwrap()));
        assert!(path.to_str().unwrap().ends_with(".claude/settings.json"));
    }

    #[test]
    fn test_apply_provider_clears_stale_credentials() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let p1 = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://api.one.com".to_string(),
            );
            f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "tok-secret".to_string());
            f
        });
        apply_provider("~/.claude", &p1).unwrap();

        let p2 = make_provider({
            let mut f = std::collections::HashMap::new();
            f.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://api.two.com".to_string(),
            );
            f
        });
        apply_provider("~/.claude", &p2).unwrap();

        let value = crate::config::read_settings("~/.claude").unwrap();
        let env = &value["env"];
        assert_eq!(env["ANTHROPIC_BASE_URL"], "https://api.two.com");
        assert!(env
            .get("ANTHROPIC_AUTH_TOKEN")
            .is_none_or(|v| v.is_null() || v.as_str() == Some("")));
    }

    #[test]
    fn test_clear_state_removes_claude_history_and_cache() {
        let dir = setup_temp_home();
        let home_dir = dir.join(".claude");
        fs::create_dir_all(&home_dir).unwrap();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        fs::write(dir.join(".claude.json"), "{}").unwrap();
        fs::write(home_dir.join("history.jsonl"), "history").unwrap();
        fs::write(home_dir.join("settings.json"), "{}").unwrap();
        fs::write(home_dir.join("keep.txt"), "keep").unwrap();

        for name in CLEAR_CONTENT_DIRS.iter().copied() {
            let target = home_dir.join(name);
            fs::create_dir_all(target.join("nested")).unwrap();
            fs::write(target.join("entry.txt"), "entry").unwrap();
            fs::write(target.join("nested").join("entry.txt"), "nested").unwrap();
        }

        let targets = clear_targets("~/.claude");
        let stats = clear::clear_targets(&targets).unwrap();

        assert_eq!(stats.removed, 20);
        assert!(!dir.join(".claude.json").exists());
        assert!(!home_dir.join("history.jsonl").exists());
        assert!(home_dir.join("settings.json").exists());
        assert!(home_dir.join("keep.txt").exists());
        for name in CLEAR_CONTENT_DIRS.iter().copied() {
            let target = home_dir.join(name);
            assert!(target.exists());
            assert!(fs::read_dir(target).unwrap().next().is_none());
        }
    }
}
