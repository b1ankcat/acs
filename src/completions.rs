//! Auto-completion setup module
//! Automatically generates and installs shell completions on first run

use std::fs;
use std::path::PathBuf;
use clap::CommandFactory;
use clap_complete::{generate_to, shells};

/// Check if completions have been installed, and install them if not
pub fn ensure_completions_installed() -> Result<(), Box<dyn std::error::Error>> {
    let marker_path = get_completions_marker_path()?;

    // Check if already installed
    if marker_path.exists() {
        return Ok(());
    }

    // First run - generate and install completions
    let shell = detect_shell();
    if let Some(shell_name) = &shell {
        install_completions()?;

        // Create marker file
        if let Some(parent) = marker_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&marker_path, "")?;

        // Show friendly message
        print_installation_message(shell_name);
    }

    Ok(())
}

/// Print a friendly message about completion installation
fn print_installation_message(shell: &str) {
    eprintln!("✓ Shell completions installed for {}", shell);

    match shell {
        "bash" => {
            eprintln!("  Restart your shell or run: source ~/.local/share/bash-completion/completions/acs");
        }
        "zsh" => {
            eprintln!("  Restart your shell to enable completions");
        }
        "fish" => {
            eprintln!("  Completions are automatically available");
        }
        _ => {
            eprintln!("  Restart your shell to enable completions");
        }
    }
    eprintln!();
}

/// Get the path to the completions marker file
fn get_completions_marker_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = home::home_dir()
        .ok_or("Could not determine home directory")?;
    Ok(home.join(".config/acs/.completions_installed"))
}

/// Install completions for the current shell
fn install_completions() -> Result<(), Box<dyn std::error::Error>> {
    let shell = detect_shell();

    match shell.as_deref() {
        Some("bash") => install_bash_completions()?,
        Some("zsh") => install_zsh_completions()?,
        Some("fish") => install_fish_completions()?,
        Some("powershell") | Some("pwsh") => install_powershell_completions()?,
        Some("elvish") => install_elvish_completions()?,
        _ => {
            // Unknown shell or couldn't detect - skip silently
            return Ok(());
        }
    }

    Ok(())
}

/// Detect the current shell
fn detect_shell() -> Option<String> {
    // Try SHELL environment variable first
    if let Ok(shell) = std::env::var("SHELL") {
        if let Some(name) = shell.split('/').last() {
            return Some(name.to_string());
        }
    }

    // On Windows, check for PowerShell
    #[cfg(windows)]
    {
        return Some("powershell".to_string());
    }

    None
}

/// Install bash completions
fn install_bash_completions() -> Result<(), Box<dyn std::error::Error>> {
    let home = home::home_dir().ok_or("Could not determine home directory")?;
    let completion_dir = home.join(".local/share/bash-completion/completions");
    fs::create_dir_all(&completion_dir)?;

    let mut cmd = crate::cli::Cli::command();
    let generated_file = generate_to(shells::Bash, &mut cmd, "acs", &completion_dir)?;

    // Rename acs.bash to acs (bash-completion expects no extension)
    let target_file = completion_dir.join("acs");
    if generated_file != target_file && generated_file.exists() {
        fs::rename(&generated_file, &target_file)?;
    }

    Ok(())
}

/// Install zsh completions
fn install_zsh_completions() -> Result<(), Box<dyn std::error::Error>> {
    let home = home::home_dir().ok_or("Could not determine home directory")?;
    let completion_dir = home.join(".local/share/zsh/site-functions");
    fs::create_dir_all(&completion_dir)?;

    let mut cmd = crate::cli::Cli::command();
    generate_to(shells::Zsh, &mut cmd, "acs", &completion_dir)?;

    // Add instruction comment to zsh completion file
    let comp_file = completion_dir.join("_acs");
    if comp_file.exists() {
        // Check if fpath is configured
        let zshrc = home.join(".zshrc");
        if zshrc.exists() {
            let content = fs::read_to_string(&zshrc)?;
            if !content.contains(".local/share/zsh/site-functions") {
                // Append fpath configuration
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .open(&zshrc)?;
                use std::io::Write;
                writeln!(file, "\n# Added by acs for completions")?;
                writeln!(file, "fpath=(~/.local/share/zsh/site-functions $fpath)")?;
                writeln!(file, "autoload -Uz compinit && compinit")?;
            }
        }
    }

    Ok(())
}

/// Install fish completions
fn install_fish_completions() -> Result<(), Box<dyn std::error::Error>> {
    let home = home::home_dir().ok_or("Could not determine home directory")?;
    let completion_dir = home.join(".config/fish/completions");
    fs::create_dir_all(&completion_dir)?;

    let mut cmd = crate::cli::Cli::command();
    generate_to(shells::Fish, &mut cmd, "acs", &completion_dir)?;

    Ok(())
}

/// Install PowerShell completions
fn install_powershell_completions() -> Result<(), Box<dyn std::error::Error>> {
    let home = home::home_dir().ok_or("Could not determine home directory")?;
    let completion_dir = home.join(".config/powershell");
    fs::create_dir_all(&completion_dir)?;

    let comp_file = completion_dir.join("acs_completion.ps1");
    let mut cmd = crate::cli::Cli::command();

    // Generate to a temp buffer
    let mut buf = Vec::new();
    clap_complete::generate(shells::PowerShell, &mut cmd, "acs", &mut buf);
    fs::write(&comp_file, buf)?;

    // Try to add to PowerShell profile
    #[cfg(windows)]
    {
        if let Ok(profile) = std::env::var("PROFILE") {
            let profile_path = PathBuf::from(profile);
            if let Ok(mut content) = fs::read_to_string(&profile_path) {
                let source_line = format!(". {}", comp_file.display());
                if !content.contains(&source_line) {
                    content.push_str(&format!("\n# Added by acs\n{}\n", source_line));
                    let _ = fs::write(&profile_path, content);
                }
            }
        }
    }

    Ok(())
}

/// Install Elvish completions
fn install_elvish_completions() -> Result<(), Box<dyn std::error::Error>> {
    let home = home::home_dir().ok_or("Could not determine home directory")?;
    let completion_dir = home.join(".config/elvish/lib");
    fs::create_dir_all(&completion_dir)?;

    let comp_file = completion_dir.join("acs_completion.elv");
    let mut cmd = crate::cli::Cli::command();

    // Generate to a temp buffer
    let mut buf = Vec::new();
    clap_complete::generate(shells::Elvish, &mut cmd, "acs", &mut buf);
    fs::write(&comp_file, buf)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        // Should not panic
        let _ = detect_shell();
    }

    #[test]
    fn test_marker_path() {
        let path = get_completions_marker_path();
        assert!(path.is_ok());
    }
}
