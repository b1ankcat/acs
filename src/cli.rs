use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "acs", version, about = "AI CLI Switch — switch between Claude Code, Codex CLI, and Gemini CLI configurations")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage Claude Code configuration
    Claude { #[command(subcommand)] action: ClaudeAction },
    /// Manage Codex CLI configuration
    Codex  { #[command(subcommand)] action: CodexAction  },
    /// Manage Gemini CLI configuration
    Gemini { #[command(subcommand)] action: GeminiAction },
    /// Show current active configuration for all tools
    Status,
    /// Import providers from a TOML config file
    Import { path: String, #[arg(long)] force: bool },
    /// Export current configuration to a TOML file
    Export { path: String },
}

// ── Per-tool field args ────────────────────────────────────────────────────

#[derive(Args, Clone, Default)]
pub struct ClaudeArgs {
    /// ANTHROPIC_BASE_URL
    #[arg(long)] pub base_url: Option<String>,
    /// ANTHROPIC_AUTH_TOKEN
    #[arg(long)] pub api_key: Option<String>,
    /// ANTHROPIC_MODEL
    #[arg(long)] pub model: Option<String>,
    /// ANTHROPIC_DEFAULT_HAIKU_MODEL
    #[arg(long)] pub haiku_model: Option<String>,
    /// ANTHROPIC_DEFAULT_SONNET_MODEL
    #[arg(long)] pub sonnet_model: Option<String>,
    /// ANTHROPIC_DEFAULT_OPUS_MODEL
    #[arg(long)] pub opus_model: Option<String>,
    /// Add a fallback URL (repeatable)
    #[arg(long, value_name = "URL")] pub add_fallback_url: Vec<String>,
    /// Remove a fallback URL (repeatable)
    #[arg(long, value_name = "URL")] pub remove_fallback_url: Vec<String>,
}

#[derive(Args, Clone, Default)]
pub struct CodexArgs {
    /// base_url
    #[arg(long)] pub base_url: Option<String>,
    /// openai_api_key
    #[arg(long)] pub api_key: Option<String>,
    /// model
    #[arg(long)] pub model: Option<String>,
    /// model_reasoning_effort
    #[arg(long)] pub reasoning_effort: Option<String>,
    /// Add a fallback URL (repeatable)
    #[arg(long, value_name = "URL")] pub add_fallback_url: Vec<String>,
    /// Remove a fallback URL (repeatable)
    #[arg(long, value_name = "URL")] pub remove_fallback_url: Vec<String>,
}

#[derive(Args, Clone, Default)]
pub struct GeminiArgs {
    /// GOOGLE_GEMINI_BASE_URL
    #[arg(long)] pub base_url: Option<String>,
    /// GEMINI_API_KEY
    #[arg(long)] pub api_key: Option<String>,
    /// GEMINI_MODEL
    #[arg(long)] pub model: Option<String>,
    /// Add a fallback URL (repeatable)
    #[arg(long, value_name = "URL")] pub add_fallback_url: Vec<String>,
    /// Remove a fallback URL (repeatable)
    #[arg(long, value_name = "URL")] pub remove_fallback_url: Vec<String>,
}

// ── Unified ProviderArgs for main.rs → HashMap conversion ─────────────────

pub struct ProviderArgs {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub add_fallback_url: Vec<String>,
    pub remove_fallback_url: Vec<String>,
}

impl From<ClaudeArgs> for ProviderArgs {
    fn from(a: ClaudeArgs) -> Self {
        Self { base_url: a.base_url, api_key: a.api_key, model: a.model,
               haiku_model: a.haiku_model, sonnet_model: a.sonnet_model,
               opus_model: a.opus_model, reasoning_effort: None,
               add_fallback_url: a.add_fallback_url, remove_fallback_url: a.remove_fallback_url }
    }
}
impl From<CodexArgs> for ProviderArgs {
    fn from(a: CodexArgs) -> Self {
        Self { base_url: a.base_url, api_key: a.api_key, model: a.model,
               reasoning_effort: a.reasoning_effort,
               haiku_model: None, sonnet_model: None, opus_model: None,
               add_fallback_url: a.add_fallback_url, remove_fallback_url: a.remove_fallback_url }
    }
}
impl From<GeminiArgs> for ProviderArgs {
    fn from(a: GeminiArgs) -> Self {
        Self { base_url: a.base_url, api_key: a.api_key, model: a.model,
               haiku_model: None, sonnet_model: None, opus_model: None, reasoning_effort: None,
               add_fallback_url: a.add_fallback_url, remove_fallback_url: a.remove_fallback_url }
    }
}

// ── Action enums ───────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ClaudeAction {
    /// List all configured providers
    List,
    /// Switch to a provider (interactive, or pass provider name directly)
    Use    { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Add a new provider (interactive, or pass all fields as arguments)
    Add    { #[arg(long, help="Provider name")] name: Option<String>, #[command(flatten)] fields: ClaudeArgs, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Remove a provider (interactive, or pass provider name directly)
    Remove { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Edit configuration for a provider (interactive, or pass fields as arguments)
    #[command(override_usage = "acs claude config [PROVIDER] [OPTIONS]")]
    Config { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(long, help="~/.claude home directory")] home: Option<String>, #[command(flatten)] fields: ClaudeArgs, #[arg(long, help="Rename this provider")] rename: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Clear local sessions, history, and cache files
    Clear  { #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Test all URLs for the active provider and select one interactively
    Test,
}

#[derive(Subcommand)]
pub enum CodexAction {
    /// List all configured providers
    List,
    /// Switch to a provider (interactive, or pass provider name directly)
    Use    { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Add a new provider (interactive, or pass all fields as arguments)
    Add    { #[arg(long, help="Provider name")] name: Option<String>, #[command(flatten)] fields: CodexArgs, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Remove a provider (interactive, or pass provider name directly)
    Remove { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Edit configuration for a provider (interactive, or pass fields as arguments)
    #[command(override_usage = "acs codex config [PROVIDER] [OPTIONS]")]
    Config { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(long, help="~/.codex home directory")] home: Option<String>, #[command(flatten)] fields: CodexArgs, #[arg(long, help="Rename this provider")] rename: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Clear local sessions, history, and cache files
    Clear  { #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Test all URLs for the active provider and select one interactively
    Test,
}

#[derive(Subcommand)]
pub enum GeminiAction {
    /// List all configured providers
    List,
    /// Switch to a provider (interactive, or pass provider name directly)
    Use    { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Add a new provider (interactive, or pass all fields as arguments)
    Add    { #[arg(long, help="Provider name")] name: Option<String>, #[command(flatten)] fields: GeminiArgs, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Remove a provider (interactive, or pass provider name directly)
    Remove { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Edit configuration for a provider (interactive, or pass fields as arguments)
    #[command(override_usage = "acs gemini config [PROVIDER] [OPTIONS]")]
    Config { #[arg(value_name="PROVIDER", help="Provider name")] provider: Option<String>, #[arg(long, help="~/.gemini home directory")] home: Option<String>, #[command(flatten)] fields: GeminiArgs, #[arg(long, help="Rename this provider")] rename: Option<String>, #[arg(short='y', long="yes", help="Skip confirmation")] yes: bool },
    /// Test all URLs for the active provider and select one interactively
    Test,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_status() {
        let cli = Cli::try_parse_from(["acs", "status"]).unwrap();
        assert!(matches!(cli.command, Command::Status));
    }

    #[test]
    fn test_parse_import() {
        let cli = Cli::try_parse_from(["acs", "import", "config.toml"]).unwrap();
        assert!(matches!(cli.command, Command::Import { path, force: false } if path == "config.toml"));
    }

    #[test]
    fn test_parse_import_force() {
        let cli = Cli::try_parse_from(["acs", "import", "config.toml", "--force"]).unwrap();
        assert!(matches!(cli.command, Command::Import { path, force: true } if path == "config.toml"));
    }

    #[test]
    fn test_parse_export() {
        let cli = Cli::try_parse_from(["acs", "export", "out.toml"]).unwrap();
        assert!(matches!(cli.command, Command::Export { path } if path == "out.toml"));
    }

    #[test]
    fn test_parse_claude_list() {
        let cli = Cli::try_parse_from(["acs", "claude", "list"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::List }));
    }

    #[test]
    fn test_parse_claude_use_direct() {
        let cli = Cli::try_parse_from(["acs", "claude", "use", "my-prov"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::Use { provider: Some(ref p), .. } } if p == "my-prov"));
    }

    #[test]
    fn test_parse_claude_add_noninteractive() {
        let cli = Cli::try_parse_from(["acs", "claude", "add", "--name", "prod", "--base-url", "https://api.anthropic.com", "--api-key", "sk", "-y"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::Add { yes: true, .. } }));
    }

    #[test]
    fn test_parse_claude_config_noninteractive() {
        let cli = Cli::try_parse_from(["acs", "claude", "config", "my-prov", "--model", "claude-opus-4-8", "-y"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::Config { yes: true, .. } }));
    }

    #[test]
    fn test_parse_claude_config_rename() {
        let cli = Cli::try_parse_from(["acs", "claude", "config", "old", "--rename", "new", "-y"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::Config { rename: Some(ref r), .. } } if r == "new"));
    }

    #[test]
    fn test_parse_claude_clear() {
        let cli = Cli::try_parse_from(["acs", "claude", "clear"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::Clear { yes: false } }));
    }

    #[test]
    fn test_parse_codex_add_with_reasoning_effort() {
        let cli = Cli::try_parse_from(["acs", "codex", "add", "--name", "n", "--base-url", "https://x", "--reasoning-effort", "high", "-y"]).unwrap();
        assert!(matches!(cli.command, Command::Codex { action: CodexAction::Add { yes: true, .. } }));
    }

    #[test]
    fn test_parse_gemini_clear_is_invalid() {
        assert!(Cli::try_parse_from(["acs", "gemini", "clear"]).is_err());
    }

    #[test]
    fn test_codex_no_haiku_model() {
        assert!(Cli::try_parse_from(["acs", "codex", "add", "--name", "n", "--haiku-model", "h"]).is_err());
    }

    #[test]
    fn test_parse_claude_haiku_model() {
        let cli = Cli::try_parse_from(["acs", "claude", "add", "--name", "n", "--base-url", "u", "--haiku-model", "h"]).unwrap();
        assert!(matches!(cli.command, Command::Claude { action: ClaudeAction::Add { .. } }));
    }

    #[test]
    fn test_gemini_no_haiku_model() {
        // gemini has no --haiku-model; it should be rejected
        assert!(Cli::try_parse_from(["acs", "gemini", "add", "--name", "n", "--haiku-model", "h"]).is_err());
    }
}
