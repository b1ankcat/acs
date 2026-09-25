mod claude;
mod clear;
mod cli;
mod codex;
mod completions;
mod config;
mod errors;
mod fields;
mod import_;
mod gemini;
mod keyring;
mod prompts;
mod provider;
mod test_cmd;

#[cfg(test)]
pub(crate) static HOME_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

use anyhow::Result;
use clap::Parser;
use cli::{ClaudeAction, CodexAction, Command, GeminiAction};
use colored::*;
use std::collections::HashMap;
use std::io::{self, Write};

use crate::errors::{AcsError, InteractiveError, ProviderError};

// Trait to unify action handling across tools
trait ToolAction {
    fn is_clear(&self) -> Option<bool>;
    fn extract_action(&self) -> ActionType;
}

enum ActionType {
    List,
    Use { provider: Option<String>, yes: bool },
    Add { name: Option<String>, fields: cli::ProviderArgs, yes: bool },
    Remove { provider: Option<String>, yes: bool },
    Config { provider: Option<String>, home: Option<String>, fields: cli::ProviderArgs, rename: Option<String>, yes: bool },
    Test,
    #[allow(dead_code)]
    Clear { yes: bool },
}

impl ToolAction for ClaudeAction {
    fn is_clear(&self) -> Option<bool> {
        match self {
            ClaudeAction::Clear { yes } => Some(*yes),
            _ => None,
        }
    }

    fn extract_action(&self) -> ActionType {
        match self {
            ClaudeAction::List => ActionType::List,
            ClaudeAction::Use { provider, yes } => ActionType::Use { provider: provider.clone(), yes: *yes },
            ClaudeAction::Add { name, fields, yes } => ActionType::Add {
                name: name.clone(),
                fields: fields.clone().into(),
                yes: *yes
            },
            ClaudeAction::Remove { provider, yes } => ActionType::Remove { provider: provider.clone(), yes: *yes },
            ClaudeAction::Config { provider, home, fields, rename, yes } => ActionType::Config {
                provider: provider.clone(),
                home: home.clone(),
                fields: fields.clone().into(),
                rename: rename.clone(),
                yes: *yes
            },
            ClaudeAction::Test => ActionType::Test,
            ClaudeAction::Clear { yes } => ActionType::Clear { yes: *yes },
        }
    }
}

impl ToolAction for CodexAction {
    fn is_clear(&self) -> Option<bool> {
        match self {
            CodexAction::Clear { yes } => Some(*yes),
            _ => None,
        }
    }

    fn extract_action(&self) -> ActionType {
        match self {
            CodexAction::List => ActionType::List,
            CodexAction::Use { provider, yes } => ActionType::Use { provider: provider.clone(), yes: *yes },
            CodexAction::Add { name, fields, yes } => ActionType::Add {
                name: name.clone(),
                fields: fields.clone().into(),
                yes: *yes
            },
            CodexAction::Remove { provider, yes } => ActionType::Remove { provider: provider.clone(), yes: *yes },
            CodexAction::Config { provider, home, fields, rename, yes } => ActionType::Config {
                provider: provider.clone(),
                home: home.clone(),
                fields: fields.clone().into(),
                rename: rename.clone(),
                yes: *yes
            },
            CodexAction::Test => ActionType::Test,
            CodexAction::Clear { yes } => ActionType::Clear { yes: *yes },
        }
    }
}

impl ToolAction for GeminiAction {
    fn is_clear(&self) -> Option<bool> {
        None // Gemini doesn't support clear
    }

    fn extract_action(&self) -> ActionType {
        match self {
            GeminiAction::List => ActionType::List,
            GeminiAction::Use { provider, yes } => ActionType::Use { provider: provider.clone(), yes: *yes },
            GeminiAction::Add { name, fields, yes } => ActionType::Add {
                name: name.clone(),
                fields: fields.clone().into(),
                yes: *yes
            },
            GeminiAction::Remove { provider, yes } => ActionType::Remove { provider: provider.clone(), yes: *yes },
            GeminiAction::Config { provider, home, fields, rename, yes } => ActionType::Config {
                provider: provider.clone(),
                home: home.clone(),
                fields: fields.clone().into(),
                rename: rename.clone(),
                yes: *yes
            },
            GeminiAction::Test => ActionType::Test,
        }
    }
}

// Generic handler for all tools
fn handle_tool<T: ToolAction>(tool_name: &str, action: T) -> Result<(), AcsError> {
    // Handle clear separately if the action is clear
    if let Some(yes) = action.is_clear() {
        let cfg = load_config_with_defaults()?;
        return cmd_clear(tool_name, cfg.get_tool(tool_name), yes);
    }

    let mut cfg = load_config_with_defaults()?;

    match action.extract_action() {
        ActionType::List => cmd_list(tool_name, cfg.get_tool(tool_name)),
        ActionType::Use { provider, yes } => cmd_use(tool_name, &mut cfg, provider.as_deref(), yes),
        ActionType::Add { name, fields, yes } => {
            cmd_add(tool_name, &mut cfg, AddProviderParams {
                name: name.as_deref(),
                cli_args: &provider_args_to_map(&fields),
                fallback_urls: &fields.add_fallback_url,
                use_keyring: fields.use_keyring,
                no_keyring: fields.no_keyring,
                yes,
            })
        }
        ActionType::Remove { provider, yes } => cmd_remove(tool_name, &mut cfg, provider.as_deref(), yes),
        ActionType::Config { provider, home, fields, rename, yes } => {
            cmd_config(tool_name, &mut cfg, ConfigProviderParams {
                provider: provider.as_deref(),
                new_home: home.as_deref(),
                cli_args: &provider_args_to_map(&fields),
                rename: rename.as_deref(),
                add_fallback: &fields.add_fallback_url,
                remove_fallback: &fields.remove_fallback_url,
                use_keyring: fields.use_keyring,
                no_keyring: fields.no_keyring,
                yes,
            })
        }
        ActionType::Test => test_cmd::run_test(tool_name, &mut cfg),
        ActionType::Clear { .. } => unreachable!(),
    }
}

fn main() -> Result<()> {
    // Auto-install shell completions on first run (silently fail if error)
    let _ = completions::ensure_completions_installed();

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
    handle_tool("claude", action)
}

fn handle_codex(action: CodexAction) -> Result<(), AcsError> {
    handle_tool("codex", action)
}

fn handle_gemini(action: GeminiAction) -> Result<(), AcsError> {
    handle_tool("gemini", action)
}

fn provider_args_to_map<'a>(f: &'a cli::ProviderArgs) -> std::collections::HashMap<&'static str, &'a str> {
    [
        ("base-url",         f.base_url.as_deref()),
        ("api-key",          f.api_key.as_deref()),
        ("model",            f.model.as_deref()),
        ("subagent-model",   f.subagent_model.as_deref()),
        ("haiku-model",      f.haiku_model.as_deref()),
        ("sonnet-model",     f.sonnet_model.as_deref()),
        ("opus-model",       f.opus_model.as_deref()),
        ("reasoning-effort", f.reasoning_effort.as_deref()),
        ("model-context-window", f.model_context_window.as_deref()),
        ("model-auto-compact-token-limit", f.model_auto_compact_token_limit.as_deref()),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|val| (k, val)))
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

/// Select a provider interactively or validate provided name
fn select_provider(
    tool_name: &str,
    tool: &config::ToolConfig,
    provider: Option<&str>,
    action: &str,
) -> Result<String, AcsError> {
    if tool.providers.is_empty() {
        return Err(ProviderError::no_providers(tool_name).into());
    }

    if let Some(p) = provider {
        if !tool.providers.contains_key(p) {
            return Err(ProviderError::not_found(p, tool_name).into());
        }
        Ok(p.to_string())
    } else if tool.providers.len() == 1 {
        Ok(tool.providers.keys().next().unwrap().clone())
    } else {
        Ok(prompts::prompt_select_provider(action, tool_name, &tool.providers, &tool.active)?)
    }
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

/// Get API key field name for a tool
fn api_key_field_for(tool_name: &str) -> &'static str {
    match tool_name {
        "claude" => "ANTHROPIC_AUTH_TOKEN",
        "codex" => "openai_api_key",
        "gemini" => "GEMINI_API_KEY",
        _ => "",
    }
}

/// Encode API key in fields if present
fn encode_api_key_in_fields(
    tool_name: &str,
    provider_name: &str,
    fields: &mut HashMap<String, String>,
    use_keyring: bool,
) {
    let key_field = api_key_field_for(tool_name);
    if !key_field.is_empty() {
        if let Some(api_key) = fields.get(key_field) {
            let encoded = keyring::encode_api_key(tool_name, provider_name, api_key, use_keyring);
            fields.insert(key_field.to_string(), encoded);
        }
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
    names.sort_unstable();

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
    let tool = cfg.get_tool(tool_name);
    let name = select_provider(tool_name, tool, provider, "use")?;

    if name != tool.active {
        if !prompts::confirm(&format!("Switch {} to provider \"{}\"?", tool_name, name), yes)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let provider_obj = provider::set_active(tool_name, cfg, &name)?;
    apply_provider_for(tool_name, &cfg.get_tool(tool_name).home, &provider_obj)?;
    config::save_config(cfg)?;
    println!("Switched {} to provider \"{}\".", tool_name, name);
    Ok(())
}

/// Parameters for adding a provider
struct AddProviderParams<'a> {
    name: Option<&'a str>,
    cli_args: &'a HashMap<&'a str, &'a str>,
    fallback_urls: &'a [String],
    use_keyring: bool,
    no_keyring: bool,
    yes: bool,
}

fn cmd_add(
    tool_name: &str,
    cfg: &mut config::AcsConfig,
    params: AddProviderParams,
) -> Result<(), AcsError> {
    let input = if let Some(n) = params.name {
        prompts::build_add_provider_fields(tool_name, n, params.cli_args, params.use_keyring, params.no_keyring)
            .map_err(|missing_arg| AcsError::from(InteractiveError::input(
                format!("--{} is required for non-interactive add", missing_arg)
            )))?
    } else {
        prompts::prompt_add_provider(tool_name)?
    };

    config::validate_provider_name(&input.name)?;

    // Encode API key if present
    let mut fields = input.fields.clone();
    encode_api_key_in_fields(tool_name, &input.name, &mut fields, input.use_keyring);

    fields::validate_provider_fields(tool_name, &fields)
        .map_err(InteractiveError::input)?;

    let tool = cfg.get_tool(tool_name);
    let exists = tool.providers.contains_key(&input.name);

    // Show preview and confirm
    if params.name.is_some() {
        println!("\n  Provider: {}", input.name);
        for f in fields::fields_for(tool_name) {
            if let Some(val) = fields.get(f.key) {
                println!("  {}: {}", f.key, if f.secret { "****" } else { val.as_str() });
            }
        }
        let msg = if exists {
            format!("Provider \"{}\" exists. Overwrite?", input.name)
        } else {
            "Add this provider?".to_string()
        };
        if !prompts::confirm(&msg, params.yes)? {
            return Err(InteractiveError::Cancelled.into());
        }
    }

    // Convert fields to HashMap<&str, &str> for provider module
    let fields_map: std::collections::HashMap<&str, &str> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let all_fallbacks = config::merge_fallback_urls(input.fallback_urls, params.fallback_urls);
    let (provider_obj, was_new) = provider::add_or_update(
        tool_name,
        cfg,
        &input.name,
        &fields_map,
        &all_fallbacks,
        params.use_keyring,
        params.no_keyring,
    )?;

    // Apply to native config if first provider or updating active
    let tool = cfg.get_tool(tool_name);
    let should_apply = (was_new && tool.providers.len() == 1) || tool.active == input.name;
    let home = tool.home.clone();

    if should_apply {
        cfg.get_tool_mut(tool_name).active = input.name.clone();
        apply_provider_for(tool_name, &home, &provider_obj)?;
    }

    config::save_config(cfg)?;
    println!("Added provider \"{}\" for {}.", input.name, tool_name);
    Ok(())
}

fn cmd_remove(tool_name: &str, cfg: &mut config::AcsConfig, provider: Option<&str>, yes: bool) -> Result<(), AcsError> {
    let tool = cfg.get_tool(tool_name);

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

    provider::remove(tool_name, cfg, &name)?;
    config::save_config(cfg)?;
    println!("Removed provider \"{}\" from {}.", name, tool_name);
    Ok(())
}

/// Show config change summary
fn show_config_summary(
    tool: &config::ToolConfig,
    home_opt: &Option<String>,
    pending: &[(String, String, String)],
    add_fallback: &[String],
    remove_fallback: &[String],
    rename: Option<&str>,
    name: &str,
) {
    if let Some(ref h) = home_opt {
        println!("  Home directory: {} -> {}", tool.home, h);
    }
    for (key, old, new) in pending {
        let s = prompts::is_secret_key(key);
        let old_d = if s && !old.is_empty() { "****" } else { old.as_str() };
        let new_d = if s && !new.is_empty() { "****" } else { new.as_str() };
        println!("  {}: {} -> {}", key, old_d, new_d);
    }
    for url in add_fallback {
        println!("  + fallback: {}", url);
    }
    for url in remove_fallback {
        println!("  - fallback: {}", url);
    }
    if let Some(new_name) = rename {
        println!("  rename: {} -> {}", name, new_name);
    }
}

/// Parameters for configuring a provider
struct ConfigProviderParams<'a> {
    provider: Option<&'a str>,
    new_home: Option<&'a str>,
    cli_args: &'a HashMap<&'a str, &'a str>,
    rename: Option<&'a str>,
    add_fallback: &'a [String],
    remove_fallback: &'a [String],
    use_keyring: bool,
    no_keyring: bool,
    yes: bool,
}

fn cmd_config(
    tool_name: &str,
    cfg: &mut config::AcsConfig,
    params: ConfigProviderParams,
) -> Result<(), AcsError> {
    let tool = cfg.get_tool_mut(tool_name);

    let name = if params.provider.is_none() {
        cmd_list(tool_name, tool)?;
        prompts::prompt_select_provider("config", tool_name, &tool.providers, &tool.active)?
    } else {
        select_provider(tool_name, tool, params.provider, "config")?
    };

    let is_active = name == tool.active;
    let non_interactive = params.new_home.is_some() || !params.cli_args.is_empty() || params.rename.is_some()
        || !params.add_fallback.is_empty() || !params.remove_fallback.is_empty();

    if non_interactive {
        let p = tool.providers.get(&name)
            .ok_or_else(|| ProviderError::not_found(&name, tool_name))?
            .clone();

        let (home_opt, pending) = prompts::build_config_edits(tool_name, &p, &tool.home, params.new_home, params.cli_args);
        let mut updated_fields = p.fields.clone();

        // Apply pending changes
        let api_key_field = api_key_field_for(tool_name);
        let mut api_key_updated = false;

        for (key, _, new_value) in &pending {
            if !api_key_field.is_empty() && key == api_key_field && !new_value.starts_with("keyring:") {
                api_key_updated = true;
            }
            updated_fields.insert(key.clone(), new_value.clone());
        }

        // Encode API key if updated
        if api_key_updated {
            let use_kr = prompts::prompt_use_keyring(params.use_keyring, params.no_keyring)?;
            if use_kr {
                encode_api_key_in_fields(tool_name, &name, &mut updated_fields, true);
            }
        }

        fields::validate_provider_fields(tool_name, &updated_fields)
            .map_err(InteractiveError::input)?;

        // Handle rename
        if let Some(new_name) = params.rename {
            config::validate_provider_name(new_name)?;
            if tool.providers.contains_key(new_name) {
                return Err(ProviderError::InvalidName(new_name.to_string()).into());
            }
        }

        let has_changes = home_opt.is_some()
            || !pending.is_empty()
            || params.rename.is_some()
            || !params.add_fallback.is_empty()
            || !params.remove_fallback.is_empty();
        if !has_changes {
            println!("No changes made.");
            return Ok(());
        }

        // Show summary
        show_config_summary(tool, &home_opt, &pending, params.add_fallback, params.remove_fallback, params.rename, &name);

        if !prompts::confirm("Apply these changes?", params.yes)? {
            println!("Cancelled.");
            return Ok(());
        }

        // Apply
        if let Some(h) = home_opt {
            tool.home = h;
        }
        let p = tool.providers.get_mut(&name)
            .expect("provider existence validated above");

        // Use updated_fields which may contain encoded API key
        p.fields = updated_fields;

        config::add_fallback_urls(p, params.add_fallback);
        config::remove_fallback_urls(p, params.remove_fallback);

        if let Some(new_name) = params.rename {
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
        println!("Updated configuration for {}:{}.", tool_name, params.rename.unwrap_or(&name));
    } else {
        // Interactive path
        let (home_opt, changed) = prompts::prompt_config_edit(
            tool_name,
            &tool.home,
            Some(tool.providers.get_mut(&name)
                .ok_or_else(|| ProviderError::not_found(&name, tool_name))?),
            params.yes,
        )?;

        if !changed {
            return Ok(());
        }

        let provider = tool.providers.get(&name)
            .ok_or_else(|| ProviderError::not_found(&name, tool_name))?;
        fields::validate_provider_fields(tool_name, &provider.fields)
            .map_err(InteractiveError::input)?;

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
        config::Provider { fields, fallback_urls: vec![] }
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
            fallback_urls: vec![],
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

    #[test]
    fn test_cmd_config_updates_fallback_urls_without_field_edits() {
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
                f
            }),
        );

        let args = std::collections::HashMap::new();
        cmd_config(
            "claude",
            &mut cfg,
            ConfigProviderParams {
                provider: Some("prod"),
                new_home: None,
                cli_args: &args,
                rename: None,
                add_fallback: &["https://backup.example.com".to_string()],
                remove_fallback: &[],
                use_keyring: false,
                no_keyring: false,
                yes: true,
            }
        )
        .unwrap();

        assert_eq!(
            cfg.claude.providers["prod"].fallback_urls,
            vec!["https://backup.example.com"]
        );
    }

    #[test]
    fn test_cmd_add_codex_writes_default_context_limits() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = config::AcsConfig::default();
        cfg.codex.home = "~/.codex".to_string();
        let args = [("base-url", "https://api.example.com")].into_iter().collect();
        cmd_add("codex", &mut cfg, AddProviderParams {
            name: Some("prod"),
            cli_args: &args,
            fallback_urls: &[],
            use_keyring: false,
            no_keyring: false,
            yes: true,
        }).unwrap();

        let provider = &cfg.codex.providers["prod"];
        assert_eq!(provider.get("model_context_window"), Some("1000000"));
        assert_eq!(provider.get("model_auto_compact_token_limit"), Some("900000"));

        let native = codex::read_config("~/.codex").unwrap();
        assert_eq!(native["model_context_window"].as_integer(), Some(1_000_000));
        assert_eq!(native["model_auto_compact_token_limit"].as_integer(), Some(900_000));
    }

    #[test]
    fn test_cmd_config_rejects_invalid_codex_context_limits() {
        let dir = setup_temp_home();
        let _guard = home_lock();
        env::set_var("HOME", dir.to_str().unwrap());

        let mut cfg = config::AcsConfig::default();
        cfg.codex.home = "~/.codex".to_string();
        cfg.codex.active = "prod".to_string();
        cfg.codex.providers.insert(
            "prod".to_string(),
            make_provider({
                let mut f = std::collections::HashMap::new();
                f.insert("base_url".to_string(), "https://api.example.com".to_string());
                f.insert("model_provider".to_string(), "prod".to_string());
                f
            }),
        );
        let args = [("model-context-window", "0")].into_iter().collect();

        assert!(cmd_config("codex", &mut cfg, ConfigProviderParams {
            provider: Some("prod"),
            new_home: None,
            cli_args: &args,
            rename: None,
            add_fallback: &[],
            remove_fallback: &[],
            use_keyring: false,
            no_keyring: false,
            yes: true,
        }).is_err());
        assert_eq!(cfg.codex.providers["prod"].get("model_context_window"), None);
    }

    #[test]
    fn test_fallback_url_merge() {
        let existing = vec!["https://a.com".to_string(), "https://b.com".to_string()];
        let new = vec!["https://b.com".to_string(), "https://c.com".to_string()];
        let merged = config::merge_fallback_urls(existing, &new);
        assert_eq!(merged, vec!["https://a.com", "https://b.com", "https://c.com"]);
    }

    #[test]
    fn test_add_remove_fallback_urls() {
        let mut provider = config::Provider {
            fields: std::collections::HashMap::new(),
            fallback_urls: vec!["https://a.com".to_string()],
        };

        config::add_fallback_urls(&mut provider, &["https://b.com".to_string(), "https://a.com".to_string()]);
        assert_eq!(provider.fallback_urls.len(), 2);
        assert!(provider.fallback_urls.contains(&"https://a.com".to_string()));
        assert!(provider.fallback_urls.contains(&"https://b.com".to_string()));

        config::remove_fallback_urls(&mut provider, &["https://a.com".to_string()]);
        assert_eq!(provider.fallback_urls, vec!["https://b.com"]);
    }
}
