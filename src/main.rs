mod claude;
mod clear;
mod cli;
mod codex;
mod config;
mod errors;
mod fields;
mod import_;
mod gemini;
mod prompts;

#[cfg(test)]
pub(crate) static HOME_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

use anyhow::Result;
use clap::Parser;
use cli::{ClaudeAction, CodexAction, Command, GeminiAction};
use colored::*;
use std::io::{self, Write};

use crate::errors::{AcsError, InteractiveError, ProviderError};

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        Command::Claude { action } => handle_claude(action)?,
        Command::Codex  { action } => handle_codex(action)?,
        Command::Gemini { action } => handle_gemini(action)?,
        Command::Status => cmd_status()?,
        Command::Import { path, force } => cmd_import(&path, force)?,
        Command::Export { path } => cmd_export(&path)?,
    }
    Ok(())
}

fn handle_claude(action: ClaudeAction) -> Result<(), AcsError> {
    if let ClaudeAction::Clear { yes } = action {
        let cfg = load_config_with_defaults()?;
        return cmd_clear("claude", cfg.get_tool("claude"), yes);
    }
    let mut cfg = load_config_with_defaults()?;
    match action {
        ClaudeAction::List                              => cmd_list("claude", cfg.get_tool("claude")),
        ClaudeAction::Use    { provider, yes }         => cmd_use("claude", &mut cfg, provider.as_deref(), yes),
        ClaudeAction::Add    { name, fields, yes }     => cmd_add("claude", &mut cfg, name.as_deref(), &provider_args_to_map(&fields.into()), yes),
        ClaudeAction::Remove { provider, yes }         => cmd_remove("claude", &mut cfg, provider.as_deref(), yes),
        ClaudeAction::Config { provider, home, fields, rename, yes } =>
            cmd_config("claude", &mut cfg, provider.as_deref(), home.as_deref(), &provider_args_to_map(&fields.into()), rename.as_deref(), yes),
        ClaudeAction::Clear  { .. } => unreachable!(),
    }
}

fn handle_codex(action: CodexAction) -> Result<(), AcsError> {
    if let CodexAction::Clear { yes } = action {
        let cfg = load_config_with_defaults()?;
        return cmd_clear("codex", cfg.get_tool("codex"), yes);
    }
    let mut cfg = load_config_with_defaults()?;
    match action {
        CodexAction::List                              => cmd_list("codex", cfg.get_tool("codex")),
        CodexAction::Use    { provider, yes }         => cmd_use("codex", &mut cfg, provider.as_deref(), yes),
        CodexAction::Add    { name, fields, yes }     => cmd_add("codex", &mut cfg, name.as_deref(), &provider_args_to_map(&fields.into()), yes),
        CodexAction::Remove { provider, yes }         => cmd_remove("codex", &mut cfg, provider.as_deref(), yes),
        CodexAction::Config { provider, home, fields, rename, yes } =>
            cmd_config("codex", &mut cfg, provider.as_deref(), home.as_deref(), &provider_args_to_map(&fields.into()), rename.as_deref(), yes),
        CodexAction::Clear  { .. } => unreachable!(),
    }
}

fn handle_gemini(action: GeminiAction) -> Result<(), AcsError> {
    let mut cfg = load_config_with_defaults()?;
    match action {
        GeminiAction::List                              => cmd_list("gemini", cfg.get_tool("gemini")),
        GeminiAction::Use    { provider, yes }         => cmd_use("gemini", &mut cfg, provider.as_deref(), yes),
        GeminiAction::Add    { name, fields, yes }     => cmd_add("gemini", &mut cfg, name.as_deref(), &provider_args_to_map(&fields.into()), yes),
        GeminiAction::Remove { provider, yes }         => cmd_remove("gemini", &mut cfg, provider.as_deref(), yes),
        GeminiAction::Config { provider, home, fields, rename, yes } =>
            cmd_config("gemini", &mut cfg, provider.as_deref(), home.as_deref(), &provider_args_to_map(&fields.into()), rename.as_deref(), yes),
    }
}

fn provider_args_to_map<'a>(f: &'a cli::ProviderArgs) -> std::collections::HashMap<&'static str, &'a str> {
    [
        ("base-url",         f.base_url.as_deref()),
        ("api-key",          f.api_key.as_deref()),
        ("model",            f.model.as_deref()),
        ("haiku-model",      f.haiku_model.as_deref()),
        ("sonnet-model",     f.sonnet_model.as_deref()),
        ("opus-model",       f.opus_model.as_deref()),
        ("reasoning-effort", f.reasoning_effort.as_deref()),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|v| (k, v)))
    .collect()
}

fn load_config_with_defaults() -> Result<config::AcsConfig, AcsError> {
    let mut cfg = match config::load_config() {
        Ok(c) => c,
        Err(AcsError::Config(errors::ConfigError::Load { source, .. }))
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            // First run: no config yet — bootstrap from native tool configs
            let mut c = config::AcsConfig::default();
            config::auto_import_defaults(&mut c)?;
            c
        }
        Err(e) => return Err(e),
    };
    config::ensure_tool_defaults(&mut cfg);
    config::save_config(&cfg)?;
    Ok(cfg)
}

fn apply_provider_for(
    tool_name: &str,
    home: &str,
    provider: &config::Provider,
) -> Result<(), AcsError> {
    match tool_name {
        "claude" => claude::apply_provider(home, provider),
        "codex" => codex::apply_provider(home, provider),
        "gemini" => gemini::apply_provider(home, provider),
        _ => unreachable!(),
    }
}

fn clear_targets_for(tool_name: &str, home: &str) -> Vec<clear::ClearTarget> {
    match tool_name {
        "claude" => claude::clear_targets(home),
        "codex" => codex::clear_targets(home),
        _ => unreachable!(),
    }
}

fn cmd_list(tool_name: &str, tool: &config::ToolConfig) -> Result<(), AcsError> {
    if tool.providers.is_empty() {
        println!("No providers configured for {}.", tool_name);
        return Ok(());
    }

    let mut names: Vec<&String> = tool.providers.keys().collect();
    names.sort();

    for name in names {
        let provider = &tool.providers[name];
        let active = if *name == tool.active { "*" } else { " " };
        let model = provider.model(tool_name).unwrap_or("");
        let url = provider.base_url().trim_end_matches('/');

        println!(
            "{} {:<12} {:<24} {}",
            active.yellow().bold(),
            name,
            model,
            url
        );
    }
    Ok(())
}

fn cmd_clear(tool_name: &str, tool: &config::ToolConfig, yes: bool) -> Result<(), AcsError> {
    let targets = clear_targets_for(tool_name, &tool.home);
    if !confirm_clear_targets(tool_name, &targets, yes)? {
        println!("Clear cancelled.");
        return Ok(());
    }

    let stats = clear::clear_targets(&targets)?;
    println!(
        "Cleared {} local state (removed {} item(s)).",
        tool_name, stats.removed
    );
    Ok(())
}

fn confirm_clear_targets(
    tool_name: &str,
    targets: &[clear::ClearTarget],
    yes: bool,
) -> Result<bool, AcsError> {
    println!("The following absolute paths will be deleted or emptied for {tool_name}:");
    for target in targets {
        let abs = target.absolute_path()?;
        match target.kind {
            clear::ClearTargetKind::FileOrDir => println!("  delete {}", abs.display()),
            clear::ClearTargetKind::DirContents => println!("  empty  {}/*", abs.display()),
        }
    }
    println!("RISK WARNING: This permanently deletes local sessions, history, caches, tasks, telemetry, and snapshots.");
    println!("RISK WARNING: This action cannot be undone.");

    if yes {
        println!("Second confirmation required. Type y and press Enter to confirm: y");
        return Ok(true);
    }

    print!("Second confirmation required. Type y and press Enter to confirm: ");
    io::stdout()
        .flush()
        .map_err(|e| InteractiveError::input(e.to_string()))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| InteractiveError::input(e.to_string()))?;

    Ok(input.trim() == "y")
}

fn cmd_use(tool_name: &str, cfg: &mut config::AcsConfig, provider: Option<&str>, yes: bool) -> Result<(), AcsError> {
    let tool = cfg.get_tool_mut(tool_name);

    if tool.providers.is_empty() {
        return Err(ProviderError::no_providers(tool_name).into());
    }

    let name = if let Some(p) = provider {
        if !tool.providers.contains_key(p) {
            return Err(ProviderError::not_found(p, tool_name).into());
        }
        p.to_string()
    } else if tool.providers.len() == 1 {
        tool.providers.keys().next().unwrap().clone()
    } else {
        prompts::prompt_select_provider("use", tool_name, &tool.providers, &tool.active)?
    };

    if name != tool.active {
        if !prompts::confirm(&format!("Switch {} to provider \"{}\"?", tool_name, name), yes)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    tool.active = name.clone();
    apply_provider_for(tool_name, &tool.home, tool.providers.get(&name)
        .ok_or_else(|| ProviderError::not_found(&name, tool_name))?)?;
    config::save_config(cfg)?;
    println!("Switched {} to provider \"{}\".", tool_name, name);
    Ok(())
}

fn cmd_add(
    tool_name: &str,
    cfg: &mut config::AcsConfig,
    name: Option<&str>,
    cli_args: &std::collections::HashMap<&str, &str>,
    yes: bool,
) -> Result<(), AcsError> {
    let input = if let Some(n) = name {
        prompts::build_add_provider_fields(tool_name, n, cli_args)
            .map_err(|missing_arg| AcsError::from(InteractiveError::input(
                format!("--{} is required for non-interactive add", missing_arg)
            )))?
    } else {
        prompts::prompt_add_provider(tool_name)?
    };

    config::validate_provider_name(&input.name)?;

    let tool = cfg.get_tool_mut(tool_name);

    if tool.providers.contains_key(&input.name) {
        // Show what will be written before asking to overwrite
        if name.is_some() {
            println!("\n  Provider: {}", input.name);
            for f in fields::fields_for(tool_name) {
                if let Some(val) = input.fields.get(f.key) {
                    println!("  {}: {}", f.key, if f.secret { "****" } else { val.as_str() });
                }
            }
        }
        let overwrite = prompts::confirm(
            &format!("Provider \"{}\" already exists for {}. Overwrite?", input.name, tool_name),
            yes,
        )?;
        if !overwrite {
            return Err(InteractiveError::Cancelled.into());
        }
    } else if name.is_some() {
        // Non-interactive new provider: show summary and confirm
        println!("\n  Provider: {}", input.name);
        for f in fields::fields_for(tool_name) {
            if let Some(val) = input.fields.get(f.key) {
                println!("  {}: {}", f.key, if f.secret { "****" } else { val.as_str() });
            }
        }
        if !prompts::confirm("Add this provider?", yes)? {
            return Err(InteractiveError::Cancelled.into());
        }
    }

    let was_new = !tool.providers.contains_key(&input.name);
    let provider = config::Provider { fields: input.fields };
    tool.providers.insert(input.name.clone(), provider.clone());

    if was_new && tool.providers.len() == 1 {
        tool.active = input.name.clone();
        apply_provider_for(tool_name, &tool.home, &provider)?;
    } else if !was_new && tool.active == input.name {
        // Overwriting the active provider — propagate new fields to the tool's native config.
        apply_provider_for(tool_name, &tool.home, &provider)?;
    }

    config::save_config(cfg)?;
    println!("Added provider \"{}\" for {}.", input.name, tool_name);
    Ok(())
}

fn cmd_remove(tool_name: &str, cfg: &mut config::AcsConfig, provider: Option<&str>, yes: bool) -> Result<(), AcsError> {
    let tool = cfg.get_tool_mut(tool_name);

    let removable: Vec<String> = tool
        .providers
        .keys()
        .filter(|name| *name != "default" && *name != &tool.active)
        .cloned()
        .collect();

    if removable.is_empty() {
        return Err(ProviderError::no_removable(tool_name).into());
    }

    let name = if let Some(p) = provider {
        if !removable.contains(&p.to_string()) {
            return Err(ProviderError::not_found(p, tool_name).into());
        }
        p.to_string()
    } else {
        prompts::prompt_remove_provider(tool_name, &removable)?
    };

    if !prompts::confirm(&format!("Remove provider \"{}\" from {}?", name, tool_name), yes)? {
        println!("Cancelled.");
        return Ok(());
    }

    tool.providers.remove(&name)
        .ok_or_else(|| ProviderError::not_found(&name, tool_name))?;

    config::save_config(cfg)?;
    println!("Removed provider \"{}\" from {}.", name, tool_name);
    Ok(())
}

fn cmd_config(
    tool_name: &str,
    cfg: &mut config::AcsConfig,
    provider: Option<&str>,
    new_home: Option<&str>,
    cli_args: &std::collections::HashMap<&str, &str>,
    rename: Option<&str>,
    yes: bool,
) -> Result<(), AcsError> {
    let tool = cfg.get_tool_mut(tool_name);

    if tool.providers.is_empty() {
        return Err(ProviderError::no_providers(tool_name).into());
    }

    let name = if let Some(p) = provider {
        if !tool.providers.contains_key(p) {
            return Err(ProviderError::not_found(p, tool_name).into());
        }
        p.to_string()
    } else {
        cmd_list(tool_name, tool)?;
        prompts::prompt_select_provider("config", tool_name, &tool.providers, &tool.active)?
    };

    let is_active = name == tool.active;
    let non_interactive = new_home.is_some() || !cli_args.is_empty() || rename.is_some();

    if non_interactive {
        let p = tool.providers.get(&name)
            .ok_or_else(|| ProviderError::not_found(&name, tool_name))?
            .clone();

        let (home_opt, pending) = prompts::build_config_edits(tool_name, &p, &tool.home, new_home, cli_args);

        // Handle rename
        if let Some(new_name) = rename {
            config::validate_provider_name(new_name)?;
            if tool.providers.contains_key(new_name) {
                return Err(ProviderError::InvalidName(new_name.to_string()).into());
            }
        }

        let has_changes = home_opt.is_some() || !pending.is_empty() || rename.is_some();
        if !has_changes {
            println!("No changes made.");
            return Ok(());
        }

        // Show summary
        if let Some(ref h) = home_opt {
            println!("  Home directory: {} -> {}", tool.home, h);
        }
        for (key, old, new) in &pending {
            let s = prompts::is_secret_key(key);
            let old_d = if s && !old.is_empty() { "****" } else { old.as_str() };
            let new_d = if s && !new.is_empty() { "****" } else { new.as_str() };
            println!("  {}: {} -> {}", key, old_d, new_d);
        }
        if let Some(new_name) = rename {
            println!("  rename: {} -> {}", name, new_name);
        }

        if !prompts::confirm("Apply these changes?", yes)? {
            println!("Cancelled.");
            return Ok(());
        }

        // Apply
        if let Some(h) = home_opt {
            tool.home = h;
        }
        let p = tool.providers.get_mut(&name)
            .expect("provider existence validated above");
        for (key, _, new_val) in pending {
            p.fields.insert(key, new_val);
        }

        if let Some(new_name) = rename {
            let p = tool.providers.remove(&name)
                .ok_or_else(|| ProviderError::not_found(&name, tool_name))?;
            if is_active {
                tool.active = new_name.to_string();
            }
            tool.providers.insert(new_name.to_string(), p);
        }

        if is_active {
            let active_p = config::get_active_provider(tool)
                .ok_or_else(|| ProviderError::not_found(&tool.active, tool_name))?
                .clone();
            apply_provider_for(tool_name, &tool.home, &active_p)?;
        }

        config::save_config(cfg)?;
        println!("Updated configuration for {}:{}.", tool_name, rename.unwrap_or(&name));
    } else {
        // Interactive path
        let (home_opt, changed) = prompts::prompt_config_edit(
            tool_name,
            &tool.home,
            Some(tool.providers.get_mut(&name)
                .ok_or_else(|| ProviderError::not_found(&name, tool_name))?),
            yes,
        )?;

        if !changed {
            return Ok(());
        }

        if let Some(h) = home_opt {
            tool.home = h;
        }

        if is_active {
            let provider = config::get_active_provider(tool)
                .ok_or_else(|| ProviderError::not_found(&tool.active, tool_name))?
                .clone();
            apply_provider_for(tool_name, &tool.home, &provider)?;
        }

        config::save_config(cfg)?;
        println!("Updated configuration for {}:{}.", tool_name, name);
    }

    Ok(())
}

fn status_line(label: &str, tool: &config::ToolConfig) -> String {
    let info = if tool.providers.is_empty() {
        "not configured".to_string()
    } else if let Some(p) = config::get_active_provider(tool) {
        format!("{} ({})", tool.active, p.base_url())
    } else {
        format!("unknown ({})", tool.active)
    };
    format!("{} {}", label.bold(), info)
}

fn cmd_status() -> Result<(), AcsError> {
    let cfg = load_config_with_defaults()?;
    println!("{}", status_line("Claude:", &cfg.claude));
    println!("{}", status_line("Codex:", &cfg.codex));
    println!("{}", status_line("Gemini:", &cfg.gemini));
    Ok(())
}

fn cmd_import(path: &str, force: bool) -> Result<(), AcsError> {
    let mut cfg = load_config_with_defaults()?;
    let stats = import_::import_from_toml(path, &mut cfg, force)?;
    config::save_config(&cfg)?;
    println!(
        "Imported {} provider(s), skipped {} (already exists).",
        stats.imported, stats.skipped
    );
    Ok(())
}

fn cmd_export(path: &str) -> Result<(), AcsError> {
    let cfg = load_config_with_defaults()?;
    let content = toml::to_string_pretty(&cfg)
        .map_err(|e| errors::ConfigError::serialize(e.to_string()))?;
    std::fs::write(path, content)
        .map_err(|e| errors::ConfigError::save(std::path::Path::new(path), e))?;
    println!("Exported configuration to {}.", path);
    println!("Warning: exported file contains secret API keys — handle with care.");
    Ok(())
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

    fn setup_temp_home() -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = env::temp_dir().join(format!("acp_main_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".config")).unwrap();
        env::set_current_dir(&dir).unwrap();
        dir
    }

    fn make_provider(fields: std::collections::HashMap<String, String>) -> config::Provider {
        config::Provider { fields }
    }

    #[test]
    fn test_cmd_status_no_providers() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());
        let result = cmd_status();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_status_with_configured_providers() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = config::AcsConfig::default();
        cfg.claude.home = "~/.claude".to_string();
        cfg.claude.providers.insert(
            "my-claude".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.anthropic.com".to_string());
                f
            }),
        );
        cfg.claude.active = "my-claude".to_string();
        cfg.codex.home = "~/.codex".to_string();
        cfg.codex.providers.insert(
            "my-codex".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("base_url".to_string(), "https://api.openai.com".to_string());
                f
            }),
        );
        cfg.codex.active = "my-codex".to_string();
        config::save_config(&cfg).unwrap();

        let result = cmd_status();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_list_no_providers() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let tool = config::ToolConfig::default();
        let result = cmd_list("claude", &tool);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_list_with_providers() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut tool = config::ToolConfig {
            home: "~/.claude".to_string(),
            active: "p1".to_string(),
            providers: std::collections::HashMap::new(),
        };
        tool.providers.insert(
            "p1".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.anthropic.com".to_string());
                f.insert("ANTHROPIC_MODEL".to_string(), "claude-sonnet-4-6".to_string());
                f
            }),
        );
        let result = cmd_list("claude", &tool);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_tool_list() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let result = handle_claude(cli::ClaudeAction::List);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_provider_logic() {
        let mut tool = config::ToolConfig::default();
        let p = config::Provider {
            fields: {
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.example.com".to_string());
                f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-key-123".to_string());
                f
            },
        };
        tool.providers.insert("new-prov".to_string(), p);
        assert_eq!(tool.providers.len(), 1);
        let p = &tool.providers["new-prov"];
        assert_eq!(p.get("ANTHROPIC_AUTH_TOKEN"), Some("sk-key-123"));
    }

    #[test]
    fn test_duplicate_detection() {
        let mut tool = config::ToolConfig {
            home: String::new(),
            active: "dup-prov".to_string(),
            providers: std::collections::HashMap::new(),
        };
        tool.providers.insert(
            "dup-prov".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.example.com".to_string());
                f
            }),
        );
        assert!(tool.providers.contains_key("dup-prov"));
        assert!(!tool.providers.contains_key("new-prov"));
    }

    #[test]
    fn test_cmd_export_roundtrip() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = config::AcsConfig::default();
        cfg.claude.home = "~/.claude".to_string();
        cfg.claude.active = "prod".to_string();
        cfg.claude.providers.insert(
            "prod".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.anthropic.com".to_string());
                f.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-secret".to_string());
                f
            }),
        );
        config::save_config(&cfg).unwrap();

        let export_path = dir.join("export.toml");
        let result = cmd_export(export_path.to_str().unwrap());
        assert!(result.is_ok());

        // Re-import and verify
        let mut fresh = config::AcsConfig::default();
        import_::import_from_toml(export_path.to_str().unwrap(), &mut fresh, false).unwrap();
        assert_eq!(fresh.claude.providers.len(), 1);
        assert_eq!(
            fresh.claude.providers["prod"].base_url(),
            "https://api.anthropic.com"
        );
    }

    #[test]
    fn test_cmd_use_noninteractive_not_found() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = config::AcsConfig::default();
        cfg.claude.home = "~/.claude".to_string();
        cfg.claude.providers.insert(
            "existing".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.anthropic.com".to_string());
                f
            }),
        );
        config::save_config(&cfg).unwrap();

        let result = cmd_use("claude", &mut cfg, Some("nonexistent"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_remove_noninteractive_not_found() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = config::AcsConfig::default();
        cfg.claude.home = "~/.claude".to_string();
        cfg.claude.active = "active".to_string();
        cfg.claude.providers.insert(
            "active".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://api.anthropic.com".to_string());
                f
            }),
        );
        config::save_config(&cfg).unwrap();

        // "active" is not in removable list
        let result = cmd_remove("claude", &mut cfg, Some("active"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_switch_active_provider() {
        let mut tool = config::ToolConfig {
            home: String::new(),
            active: "first".to_string(),
            providers: std::collections::HashMap::new(),
        };
        tool.providers.insert(
            "first".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://first.example.com".to_string());
                f
            }),
        );
        tool.providers.insert(
            "second".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("ANTHROPIC_BASE_URL".to_string(), "https://second.example.com".to_string());
                f
            }),
        );
        assert_eq!(tool.active, "first");
        tool.active = "second".to_string();
        assert_eq!(tool.active, "second");
    }
}
