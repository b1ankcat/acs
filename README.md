# 🔀 acs — AI CLI Switch

> Switch between Claude Code, Codex CLI, and Gemini CLI providers in one command.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org)

---

## ✨ Features

- 🔄 **Multi-tool support** — manages Claude Code, OpenAI Codex CLI, and Gemini CLI from one place
- 📋 **Named providers** — store multiple API endpoints/keys per tool and switch between them instantly
- 🔐 **Secure key storage** — optionally encrypt API keys in system keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- 🌐 **Fallback URLs** — configure backup endpoints per provider; benchmark and switch with `test`
- 🖥️ **Interactive-first CLI** — guided TUI prompts make configuration intuitive; pass `--flag` arguments for automation and CI/CD workflows
- 💾 **Import / Export** — share provider configs across machines via TOML files
- 🧹 **Clear** — wipe local sessions, history, and caches for Claude & Codex with a single command
- 🔍 **Status** — see the active provider for every tool at a glance

---

## 📦 Installation

### From source

```bash
cargo install --path .
```

### Pre-built binaries

Download the latest release from the [Releases](../../releases) page.

> Shell completions are auto-installed on first run — just use `acs` and restart your shell.

---

## 🚀 Quick Start

### Interactive Mode (Recommended)

Simply run commands without arguments for guided prompts:

```bash
# Interactive: add a provider with step-by-step prompts
acs claude add

# Interactive: switch providers from a list
acs claude use

# See what's currently active
acs status

# List all configured providers
acs claude list
```

### Scriptable Mode (for Automation)

Pass all arguments for non-interactive execution:

```bash
# Non-interactive: add a provider for automation/CI
acs claude add --name work \
  --base-url https://api.anthropic.com \
  --api-key sk-ant-... \
  -y

# Non-interactive: switch to a specific provider
acs claude use work -y
```

---

## 📖 Usage

```
acs <TOOL> <COMMAND> [OPTIONS]
```

`TOOL` is one of `claude`, `codex`, or `gemini`.

### Commands

| Command | Description |
|---|---|
| `list` | List all configured providers for the tool |
| `use [PROVIDER]` | Switch to a provider (interactive if omitted) |
| `add [--name NAME] [--flags…]` | Add a new provider (interactive if no `--name`) |
| `remove [PROVIDER]` | Remove a non-active provider |
| `config [PROVIDER] [--flags…]` | Edit an existing provider's fields |
| `test` | Benchmark all URLs for the active provider and select one interactively |
| `status` | Show active provider for all tools |
| `import <FILE>` | Import providers from a TOML file |
| `export <FILE>` | Export all providers to a TOML file |

`claude` and `codex` also support `clear` to delete local sessions and caches.

### Provider flags

| Flag | Claude | Codex | Gemini |
|---|:---:|:---:|:---:|
| `--base-url` | ✅ | ✅ | ✅ |
| `--api-key` | ✅ | ✅ | ✅ |
| `--model` | ✅ | ✅ | ✅ |
| `--haiku-model` / `--sonnet-model` / `--opus-model` | ✅ | — | — |
| `--reasoning-effort` | — | ✅ | — |
| `--model-context-window` | — | ✅ (default `1000000`) | — |
| `--model-auto-compact-token-limit` | — | ✅ (default `900000`) | — |
| `--subagent-model` | ✅ | — | — |
| `--add-fallback-url` | ✅ | ✅ | ✅ |
| `--remove-fallback-url` | ✅ | ✅ | ✅ |
| `--use-keyring` | ✅ | ✅ | ✅ |
| `--no-keyring` | ✅ | ✅ | ✅ |

### Examples

#### Interactive Usage

```bash
# Interactive: guided prompts for all fields
acs claude add
acs claude config
acs claude use
acs claude remove
```

#### Scriptable Usage (CI/CD)

```bash
# Non-interactive: all arguments provided
acs claude add --name prod \
  --base-url https://api.anthropic.com \
  --api-key  sk-ant-... \
  --model    claude-opus-4-8 \
  --subagent-model claude-haiku-4-5 \
  --add-fallback-url https://api-backup.example.com \
  -y

# Benchmark all URLs for the active provider and pick the fastest
acs claude test

# Add / remove fallback URLs on an existing provider
acs claude config prod --add-fallback-url https://api-backup.example.com
acs claude config prod --remove-fallback-url https://api-backup.example.com

# Configure Codex context and auto-compaction limits
acs codex add --name work --base-url https://api.openai.com/v1 \
  --model-context-window 1000000 \
  --model-auto-compact-token-limit 900000 -y
acs codex config work --model-context-window 1200000 -y

# Switch providers
acs claude use prod

# Rename a provider
acs claude config staging --rename prod -y

# Export and share your config
acs export providers.toml

# Import on another machine
acs import providers.toml

# Secure API key storage with system keyring
acs claude add --name secure --use-keyring  # Encrypt key in keyring
acs codex config prod --api-key sk-new... --use-keyring  # Update and encrypt
acs gemini add --name plaintext --no-keyring  # Force plaintext storage
```

#### API Key Security

`acs` supports encrypting API keys in your system's native keyring:

- **macOS**: Keychain
- **Windows**: Credential Manager  
- **Linux**: Secret Service (GNOME Keyring, KWallet)

When adding or configuring a provider:

```bash
# Interactive mode: prompts whether to use keyring
acs claude add

# Force keyring encryption
acs claude add --name prod --api-key sk-... --use-keyring

# Force plaintext storage
acs claude add --name dev --api-key sk-... --no-keyring
```

If keyring is unavailable, `acs` automatically falls back to plaintext storage with a warning.

Encrypted keys are stored as `keyring:acs:tool:provider:api-key` references in `config.toml`. Plaintext keys remain unchanged.

---

## ⚙️ Configuration

Config is stored at `~/.config/acs/config.toml` (XDG-compliant). On first run, `acs` auto-imports credentials from the native tool config files (e.g. `~/.claude/`, `~/.codex/`).

API keys can be stored in plaintext or encrypted in the system keyring. Encrypted keys appear as `keyring:acs:tool:provider:api-key` references in `config.toml`.

> ⚠️ Exported TOML files contain plaintext API keys — handle with care. Use `--use-keyring` for sensitive environments.

---

## 🤝 Contributing

1. Fork the repo and create a feature branch
2. Make your changes, add tests
3. Run `cargo test` and ensure everything passes
4. Open a Pull Request

Bug reports and feature requests are welcome via [Issues](../../issues).

---

## 📄 License

This project is licensed under the **GNU General Public License v3.0**.  
See [LICENSE](LICENSE) for the full text.
