use crate::errors::InteractiveError;
use crate::fields;
use dialoguer::Input;
use std::collections::HashMap;

use crate::config::Provider;

type Result<T> = std::result::Result<T, InteractiveError>;

#[derive(Debug)]
pub struct AddProviderInput {
    pub name: String,
    pub fields: HashMap<String, String>,
}

fn input_required(prompt: &str) -> Result<String> {
    loop {
        let value: String = Input::new()
            .with_prompt(prompt)
            .interact_text()
            .map_err(|e| InteractiveError::input(e.to_string()))?;
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
        eprintln!("This field is required.");
    }
}

fn input_optional(prompt: &str) -> Result<Option<String>> {
    let value: String = Input::new()
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()
        .map_err(|e| InteractiveError::input(e.to_string()))?;
    let trimmed = value.trim().to_string();
    Ok(if trimmed.is_empty() { None } else { Some(trimmed) })
}

pub fn confirm(prompt: &str, yes: bool) -> Result<bool> {
    if yes {
        println!("{} (y/n) y", prompt);
        return Ok(true);
    }
    confirm_required(prompt)
}

pub fn confirm_required(prompt: &str) -> Result<bool> {
    loop {
        let value: String = Input::new()
            .with_prompt(format!("{} (y/n)", prompt))
            .interact_text()
            .map_err(|e| InteractiveError::input(e.to_string()))?;
        match value.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => eprintln!("Please enter y or n."),
        }
    }
}

fn mask(val: &str, secret: bool) -> &str {
    if secret && !val.is_empty() { "****" } else { val }
}

pub fn is_secret_key(key: &str) -> bool {
    fields::CLAUDE_FIELDS.iter()
        .chain(fields::CODEX_FIELDS)
        .chain(fields::GEMINI_FIELDS)
        .any(|f| f.key == key && f.secret)
}

/// Non-interactive add. `cli_args`: arg name → value (e.g. "base-url" → "https://...").
/// Returns Err with the missing `--arg-name` if a required field is absent.
pub fn build_add_provider_fields(
    tool_name: &str,
    name: &str,
    cli_args: &HashMap<&str, &str>,
) -> std::result::Result<AddProviderInput, &'static str> {
    let mut result = HashMap::new();
    for f in fields::fields_for(tool_name) {
        if f.from_name {
            result.insert(f.key.to_string(), name.to_string());
        } else if let Some(default) = f.default {
            result.insert(f.key.to_string(), default.to_string());
        } else if let Some(&val) = cli_args.get(f.arg) {
            result.insert(f.key.to_string(), val.to_string());
        } else if f.required {
            return Err(f.arg);
        }
    }
    Ok(AddProviderInput { name: name.to_string(), fields: result })
}

/// Interactive add.
pub fn prompt_add_provider(tool_name: &str) -> Result<AddProviderInput> {
    let name = input_required("Provider name:")?;
    let mut result = HashMap::new();

    for f in fields::fields_for(tool_name) {
        if f.from_name {
            result.insert(f.key.to_string(), name.clone());
            continue;
        }
        if let Some(default) = f.default {
            result.insert(f.key.to_string(), default.to_string());
            continue;
        }
        let prompt = if f.required {
            format!("{}:", f.key)
        } else {
            format!("{} (optional):", f.key)
        };
        let val = if f.required {
            Some(input_required(&prompt)?)
        } else {
            input_optional(&prompt)?
        };
        if let Some(v) = val {
            result.insert(f.key.to_string(), v);
        }
    }

    println!("\n  Provider: {}", name);
    for f in fields::fields_for(tool_name) {
        if let Some(val) = result.get(f.key) {
            println!("  {}: {}", f.key, mask(val, f.secret));
        }
    }

    if !confirm_required("Add this provider?")? {
        return Err(InteractiveError::Cancelled);
    }
    Ok(AddProviderInput { name, fields: result })
}

pub fn prompt_select_provider(
    action: &str,
    tool_name: &str,
    providers: &HashMap<String, Provider>,
    active_name: &str,
) -> Result<String> {
    if providers.is_empty() {
        return Err(InteractiveError::input(format!("No providers configured for {}.", tool_name)));
    }
    let mut names: Vec<&str> = providers.keys().map(String::as_str).collect();
    names.sort_unstable();
    let display: Vec<String> = names.iter().map(|n| {
        if *n == active_name { format!("(*) {n}") } else { format!("    {n}") }
    }).collect();
    let sel = dialoguer::Select::new()
        .with_prompt(format!("Choose provider to {}:", action))
        .items(&display)
        .default(0)
        .interact()?;
    Ok(names[sel].to_string())
}

pub fn prompt_remove_provider(tool_name: &str, removable: &[String]) -> Result<String> {
    if removable.is_empty() {
        return Err(InteractiveError::input(format!("No removable providers for {}.", tool_name)));
    }
    let sel = dialoguer::Select::new()
        .with_prompt("Choose provider to remove:")
        .items(removable)
        .default(0)
        .interact()?;
    Ok(removable[sel].clone())
}

/// Non-interactive config edits. `cli_args`: arg name → value.
/// Returns (new_home_opt, pending: Vec<(key, old, new)>).
pub fn build_config_edits(
    tool_name: &str,
    provider: &Provider,
    home: &str,
    new_home: Option<&str>,
    cli_args: &HashMap<&str, &str>,
) -> (Option<String>, Vec<(String, String, String)>) {
    let home_opt = new_home
        .filter(|h| !h.is_empty() && *h != home)
        .map(|h| h.to_string());

    let mut pending = Vec::new();
    for f in fields::fields_for(tool_name) {
        if !f.is_promptable() || f.arg.is_empty() { continue; }
        if let Some(&new_val) = cli_args.get(f.arg) {
            let old = provider.get(f.key).unwrap_or("").to_string();
            if new_val != old {
                pending.push((f.key.to_string(), old, new_val.to_string()));
            }
        }
    }
    (home_opt, pending)
}

/// Interactive config edit.
pub fn prompt_config_edit(
    tool_name: &str,
    home: &str,
    active_provider: Option<&mut Provider>,
    yes: bool,
) -> Result<(Option<String>, bool)> {
    println!("\nEditing {} settings (press Enter to keep current value):", tool_name);

    let home_opt = match input_optional(&format!("Home directory ({}):", home))? {
        Some(v) if v != home => Some(v),
        _ => None,
    };

    let mut pending: Vec<(String, String, String)> = Vec::new();
    if let Some(ref provider) = active_provider {
        for f in fields::fields_for(tool_name) {
            if !f.is_promptable() { continue; }
            let current = provider.get(f.key).unwrap_or("");
            let display = mask(current, f.secret);
            let val = input_optional(&format!("{} ({}):", f.key, display))?;
            if let Some(v) = val {
                if v != current {
                    pending.push((f.key.to_string(), current.to_string(), v));
                }
            }
        }
    }

    if home_opt.is_none() && pending.is_empty() {
        println!("No changes made.");
        return Ok((None, false));
    }

    println!("\nPending changes:");
    if let Some(ref h) = home_opt {
        println!("  Home directory: {} -> {}", home, h);
    }
    for (key, old, new) in &pending {
        let s = is_secret_key(key);
        println!("  {}: {} -> {}", key, mask(old, s), mask(new, s));
    }

    if !confirm("Apply these changes?", yes)? {
        println!("Cancelled.");
        return Ok((None, false));
    }

    if let Some(provider) = active_provider {
        for (key, _, new_val) in pending {
            provider.fields.insert(key, new_val);
        }
    }
    Ok((home_opt, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
        pairs.iter().cloned().collect()
    }

    #[test]
    fn test_mask_secret() {
        assert_eq!(mask("sk-abc", true), "****");
        assert_eq!(mask("", true), "");
        assert_eq!(mask("plain", false), "plain");
    }

    #[test]
    fn test_is_secret_key() {
        assert!(is_secret_key("ANTHROPIC_AUTH_TOKEN"));
        assert!(is_secret_key("GEMINI_API_KEY"));
        assert!(is_secret_key("openai_api_key"));
        assert!(!is_secret_key("ANTHROPIC_MODEL"));
    }

    #[test]
    fn test_build_add_claude_basic() {
        let args = make_args(&[("base-url", "https://api.anthropic.com"), ("api-key", "sk-k"), ("model", "m")]);
        let input = build_add_provider_fields("claude", "prod", &args).unwrap();
        assert_eq!(input.fields["ANTHROPIC_BASE_URL"], "https://api.anthropic.com");
        assert_eq!(input.fields["ANTHROPIC_AUTH_TOKEN"], "sk-k");
        assert_eq!(input.fields["ANTHROPIC_MODEL"], "m");
    }

    #[test]
    fn test_build_add_claude_optional_models() {
        let args = make_args(&[
            ("base-url", "https://api.anthropic.com"),
            ("haiku-model", "h"), ("sonnet-model", "s"), ("opus-model", "o"),
        ]);
        let input = build_add_provider_fields("claude", "prod", &args).unwrap();
        assert_eq!(input.fields["ANTHROPIC_DEFAULT_HAIKU_MODEL"], "h");
        assert_eq!(input.fields["ANTHROPIC_DEFAULT_SONNET_MODEL"], "s");
        assert_eq!(input.fields["ANTHROPIC_DEFAULT_OPUS_MODEL"], "o");
    }

    #[test]
    fn test_build_add_claude_missing_required() {
        let args = make_args(&[]);
        let err = build_add_provider_fields("claude", "prod", &args).unwrap_err();
        assert_eq!(err, "base-url");
    }

    #[test]
    fn test_build_add_codex() {
        let args = make_args(&[
            ("base-url", "https://api.openai.com/v1"),
            ("api-key", "sk-openai"),
            ("model", "gpt-5.5"),
            ("reasoning-effort", "high"),
        ]);
        let input = build_add_provider_fields("codex", "my-codex", &args).unwrap();
        assert_eq!(input.fields["base_url"], "https://api.openai.com/v1");
        assert_eq!(input.fields["openai_api_key"], "sk-openai");
        assert_eq!(input.fields["model"], "gpt-5.5");
        assert_eq!(input.fields["model_reasoning_effort"], "high");
        assert_eq!(input.fields["wire_api"], "responses");
        assert_eq!(input.fields["model_provider"], "my-codex");
    }

    #[test]
    fn test_build_add_gemini() {
        let args = make_args(&[
            ("base-url", "https://gemini.googleapis.com"),
            ("api-key", "g-key"),
            ("model", "gemini-2.5-pro"),
        ]);
        let input = build_add_provider_fields("gemini", "g", &args).unwrap();
        assert_eq!(input.fields["GOOGLE_GEMINI_BASE_URL"], "https://gemini.googleapis.com");
        assert_eq!(input.fields["GEMINI_API_KEY"], "g-key");
        assert_eq!(input.fields["GEMINI_MODEL"], "gemini-2.5-pro");
    }

    #[test]
    fn test_build_config_edits_detects_changes() {
        let provider = Provider {
            fields: [
                ("ANTHROPIC_BASE_URL".to_string(), "https://old.example.com".to_string()),
                ("ANTHROPIC_MODEL".to_string(), "claude-sonnet-4-6".to_string()),
            ].into(),
        };
        let args = make_args(&[("base-url", "https://new.example.com"), ("model", "claude-opus-4-8")]);
        let (home_opt, pending) = build_config_edits("claude", &provider, "~/.claude", None, &args);
        assert!(home_opt.is_none());
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|(k, _, _)| k == "ANTHROPIC_BASE_URL"));
        assert!(pending.iter().any(|(k, _, _)| k == "ANTHROPIC_MODEL"));
    }

    #[test]
    fn test_build_config_edits_no_change_when_same() {
        let provider = Provider {
            fields: [("ANTHROPIC_BASE_URL".to_string(), "https://same.example.com".to_string())].into(),
        };
        let args = make_args(&[("base-url", "https://same.example.com")]);
        let (_, pending) = build_config_edits("claude", &provider, "~/.claude", None, &args);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_build_config_edits_haiku_sonnet_opus() {
        let provider = Provider { fields: HashMap::new() };
        let args = make_args(&[("haiku-model", "h"), ("sonnet-model", "s"), ("opus-model", "o")]);
        let (_, pending) = build_config_edits("claude", &provider, "~/.claude", None, &args);
        assert_eq!(pending.len(), 3);
        assert!(pending.iter().any(|(k, _, _)| k == "ANTHROPIC_DEFAULT_HAIKU_MODEL"));
    }

    #[test]
    fn test_build_config_edits_codex_reasoning_effort() {
        let provider = Provider { fields: HashMap::new() };
        let args = make_args(&[("reasoning-effort", "high")]);
        let (_, pending) = build_config_edits("codex", &provider, "~/.codex", None, &args);
        assert!(pending.iter().any(|(k, _, _)| k == "model_reasoning_effort"));
    }
}
