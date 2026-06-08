# 🔀 acs — AI CLI Switch

> Switch between Claude Code, Codex CLI, and Gemini CLI providers in one command.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org)

---

## ✨ Features

- 🔄 **Multi-tool support** — manages Claude Code, OpenAI Codex CLI, and Gemini CLI from one place
- 📋 **Named providers** — store multiple API endpoints/keys per tool and switch between them instantly
- 🖥️ **Interactive or scriptable** — guided TUI prompts for humans, `--flag` overrides for CI/automation
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

---

## 🚀 Quick Start

```bash
# See what's active
acs status

# Add a provider for Claude Code
acs claude add --name work --base-url https://api.anthropic.com --api-key sk-ant-...

# Switch to it
acs claude use work

# List all configured providers
acs claude list
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

### Examples

```bash
# Non-interactive add (useful in CI)
acs claude add --name prod \
  --base-url https://api.anthropic.com \
  --api-key  sk-ant-... \
  --model    claude-opus-4-8 \
  -y

# Switch providers
acs claude use prod

# Rename a provider
acs claude config staging --rename prod -y

# Export and share your config
acs export providers.toml

# Import on another machine
acs import providers.toml
```

---

## ⚙️ Configuration

Config is stored at `~/.config/acs/config.toml` (XDG-compliant). On first run, `acs` auto-imports credentials from the native tool config files (e.g. `~/.claude/`, `~/.codex/`).

> ⚠️ Exported TOML files contain plaintext API keys — handle with care.

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
