//! Shell integration script injection for OSC 133 command boundary markers.
//!
//! OSC 133 is the modern standard for semantic shell prompts, supported by
//! `WezTerm`, iTerm2, Ghostty, Kitty, and Windows Terminal. The markers let us
//! know when a command starts executing and when it finishes, so we can buffer
//! output per-command and run format detection.
//!
//! Markers:
//!   `OSC 133;A ST` — prompt is being displayed
//!   `OSC 133;C ST` — command is executing, output begins
//!   `OSC 133;D;N ST` — command finished with exit code N

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use portable_pty::CommandBuilder;

/// Configure the shell command for OSC 133 marker injection.
///
/// Returns the path to a temporary init script (if one was created) so the
/// caller can clean it up on exit. Returns `None` for shells that don't need
/// a temp file or for unsupported shells.
pub fn prepare_command(cmd: &mut CommandBuilder, shell_name: &str) -> Result<Option<PathBuf>> {
    match shell_name {
        "bash" => inject_bash(cmd),
        "zsh" => inject_zsh(cmd),
        "fish" => Ok(inject_fish(cmd)),
        "pwsh" | "powershell" => inject_pwsh(cmd),
        _ => Ok(None), // Unknown shell — pure passthrough, no beautification.
    }
}

/// Clean up temporary init files created during injection.
pub fn cleanup(path: Option<&PathBuf>) {
    if let Some(p) = path {
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(p);
        } else {
            let _ = std::fs::remove_file(p);
        }
    }
}

// ---------------------------------------------------------------------------
// Per-shell injection
// ---------------------------------------------------------------------------

fn inject_bash(cmd: &mut CommandBuilder) -> Result<Option<PathBuf>> {
    // The DEBUG trap chains with any existing trap by saving and calling it.
    // We guard against firing for PROMPT_COMMAND, completions, and our own
    // functions using BASH_COMMAND inspection.
    //
    // PROMPT_COMMAND handling supports both the legacy string form and the
    // bash 5.1+ array form.
    let script = r#"# prezzy shell integration — sourced via --rcfile
# Source the user's real bashrc first.
[ -f ~/.bashrc ] && . ~/.bashrc

# --- OSC 133 markers ---
__prezzy_precmd() {
    local ec=$?
    builtin printf '\033]133;D;%d\007' "$ec"
    builtin printf '\033]133;A\007'
}
# Handle both string and array PROMPT_COMMAND (bash 5.1+).
if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=("__prezzy_precmd" "${PROMPT_COMMAND[@]}")
else
    PROMPT_COMMAND="__prezzy_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi

# Save any existing DEBUG trap so we can chain it.
__prezzy_orig_debug_trap="$(trap -p DEBUG | sed "s/^trap -- '\(.*\)' DEBUG$/\1/")"

__prezzy_preexec() {
    # Skip completions and our own functions.
    [ -n "$COMP_LINE" ] && return
    case "$BASH_COMMAND" in
        __prezzy_*|"$PROMPT_COMMAND") return ;;
    esac
    builtin printf '\033]133;C\007'
    # Chain the user's original DEBUG trap.
    [ -n "$__prezzy_orig_debug_trap" ] && eval "$__prezzy_orig_debug_trap"
}
trap '__prezzy_preexec' DEBUG
"#;
    let mut file = tempfile::Builder::new()
        .prefix("prezzy-bash-")
        .suffix(".sh")
        .tempfile()
        .context("create temp bash init script")?;
    file.write_all(script.as_bytes())?;
    // Persist the file (disable auto-delete) — PtySession::Drop handles cleanup.
    let path = file.into_temp_path().keep().map_err(|e| e.error)?;
    cmd.args(["--rcfile", &path.to_string_lossy()]);
    Ok(Some(path))
}

fn inject_zsh(cmd: &mut CommandBuilder) -> Result<Option<PathBuf>> {
    // zsh sources $ZDOTDIR/.zshrc on startup. We point ZDOTDIR at a temp
    // directory containing our wrapper that sources the real rc first.
    let dir = tempfile::Builder::new()
        .prefix("prezzy-zsh-")
        .tempdir()
        .context("create temp ZDOTDIR")?;
    let original_zdotdir = std::env::var("ZDOTDIR").unwrap_or_else(|_| {
        dirs::home_dir().map_or_else(String::new, |h| h.to_string_lossy().into_owned())
    });

    // Sanitize the ZDOTDIR path for embedding in a shell script.
    // Replace single-quotes with escaped form to prevent injection.
    let safe_zdotdir = original_zdotdir.replace('\'', "'\\''");

    let script = format!(
        r#"# prezzy shell integration for zsh
# Source the user's real zshrc first.
_prezzy_orig_zdotdir='{safe_zdotdir}'
if [[ -f "$_prezzy_orig_zdotdir/.zshrc" ]]; then
    ZDOTDIR="$_prezzy_orig_zdotdir"
    source "$_prezzy_orig_zdotdir/.zshrc"
fi

# --- OSC 133 markers ---
# zsh add-zsh-hook properly chains with existing hooks.
__prezzy_precmd() {{
    local ec=$?
    printf '\033]133;D;%d\007' "$ec"
    printf '\033]133;A\007'
}}
__prezzy_preexec() {{
    printf '\033]133;C\007'
}}
autoload -Uz add-zsh-hook 2>/dev/null
add-zsh-hook precmd  __prezzy_precmd
add-zsh-hook preexec __prezzy_preexec
"#
    );

    std::fs::write(dir.path().join(".zshrc"), script)?;
    cmd.env("ZDOTDIR", dir.path().as_os_str());
    // Persist the directory (disable auto-delete) — PtySession::Drop handles cleanup.
    let path = dir.keep();
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- cleanup --------------------------------------------------------------

    #[test]
    fn cleanup_removes_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("prezzy-test-cleanup-{}", std::process::id()));
        std::fs::write(&path, "test").unwrap();
        assert!(path.exists());

        cleanup(Some(&path));
        assert!(!path.exists());
    }

    #[test]
    fn cleanup_removes_directory() {
        let dir =
            std::env::temp_dir().join(format!("prezzy-test-cleanup-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("inner.txt"), "test").unwrap();
        assert!(dir.exists());

        cleanup(Some(&dir));
        assert!(!dir.exists());
    }

    #[test]
    fn cleanup_none_is_noop() {
        // Should not panic.
        cleanup(None);
    }

    #[test]
    fn cleanup_nonexistent_is_noop() {
        let path = std::env::temp_dir().join("prezzy-does-not-exist-99999");
        // Should not panic even if path doesn't exist.
        cleanup(Some(&path));
    }

    // -- Temp file creation security ------------------------------------------

    #[test]
    fn bash_temp_file_has_random_name() {
        let mut cmd = CommandBuilder::new("bash");
        let result = inject_bash(&mut cmd);
        let path = result.unwrap().unwrap();

        // Name should NOT contain the PID pattern (old insecure style).
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("prezzy-bash-"));
        assert!(name.ends_with(".sh"));
        // Random suffix means the name is longer than just "prezzy-bash-.sh".
        assert!(name.len() > "prezzy-bash-.sh".len());

        // Clean up.
        cleanup(Some(&path));
    }

    #[test]
    fn zsh_temp_dir_has_random_name() {
        let mut cmd = CommandBuilder::new("zsh");
        let result = inject_zsh(&mut cmd);
        let path = result.unwrap().unwrap();

        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("prezzy-zsh-"));
        assert!(path.is_dir());
        assert!(path.join(".zshrc").exists());

        // Clean up.
        cleanup(Some(&path));
    }

    #[test]
    fn fish_creates_no_temp_file() {
        let mut cmd = CommandBuilder::new("fish");
        let result = inject_fish(&mut cmd);
        assert!(result.is_none());
    }

    #[test]
    fn pwsh_temp_file_has_random_name() {
        let mut cmd = CommandBuilder::new("pwsh");
        let result = inject_pwsh(&mut cmd);
        let path = result.unwrap().unwrap();

        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("prezzy-pwsh-"));
        assert!(name.ends_with(".ps1"));

        cleanup(Some(&path));
    }

    #[test]
    fn pwsh_script_contains_osc_markers() {
        let mut cmd = CommandBuilder::new("pwsh");
        let path = inject_pwsh(&mut cmd).unwrap().unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("133;D"));
        assert!(content.contains("133;A"));
        assert!(content.contains("133;C"));
        assert!(content.contains("PSReadLine"));
        assert!(content.contains("$PROFILE"));
        // Uses [char]0x1b not backtick-e (PS 5.1 compat).
        assert!(content.contains("[char]0x1b"));
        assert!(!content.contains("`e]"));

        cleanup(Some(&path));
    }

    #[test]
    fn powershell_basename_routes_to_pwsh() {
        let mut cmd = CommandBuilder::new("powershell");
        let result = prepare_command(&mut cmd, "powershell").unwrap();
        assert!(result.is_some());
        cleanup(result.as_ref());
    }

    #[test]
    fn unknown_shell_creates_no_temp_file() {
        let mut cmd = CommandBuilder::new("unknown");
        let result = prepare_command(&mut cmd, "unknown").unwrap();
        assert!(result.is_none());
    }

    // -- Script content -------------------------------------------------------

    #[test]
    fn bash_script_contains_osc_markers() {
        let mut cmd = CommandBuilder::new("bash");
        let path = inject_bash(&mut cmd).unwrap().unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("133;A"));
        assert!(content.contains("133;C"));
        assert!(content.contains("133;D"));
        assert!(content.contains("PROMPT_COMMAND"));
        // Verify bash 5.1+ array support.
        assert!(content.contains("declare -a"));

        cleanup(Some(&path));
    }

    #[test]
    fn zsh_script_contains_osc_markers() {
        let mut cmd = CommandBuilder::new("zsh");
        let path = inject_zsh(&mut cmd).unwrap().unwrap();
        let content = std::fs::read_to_string(path.join(".zshrc")).unwrap();

        assert!(content.contains("133;A"));
        assert!(content.contains("133;C"));
        assert!(content.contains("133;D"));
        assert!(content.contains("add-zsh-hook"));

        cleanup(Some(&path));
    }
}

fn inject_fish(cmd: &mut CommandBuilder) -> Option<PathBuf> {
    // fish accepts --init-command / -C for startup code — no temp file needed.
    // fish event handlers naturally coexist, so no chaining is needed.
    let init = "\
function __prezzy_prompt --on-event fish_prompt
    printf '\\033]133;D;%d\\007' $status
    printf '\\033]133;A\\007'
end
function __prezzy_preexec --on-event fish_preexec
    printf '\\033]133;C\\007'
end";
    cmd.args(["-C", init]);
    None
}

fn inject_pwsh(cmd: &mut CommandBuilder) -> Result<Option<PathBuf>> {
    // PowerShell integration: override prompt for D/A markers, use PSReadLine
    // Enter key handler for C marker (command about to execute).
    //
    // Uses [char]0x1b for ESC — works in both Windows PowerShell 5.1 and pwsh 7+.
    // The `e escape literal is pwsh 6+ only, so we avoid it.
    //
    // Sourced via -Command ". '<path>'" which bypasses execution policy
    // (policy only restricts -File, not -Command).
    let script = r#"# prezzy shell integration for PowerShell
# Source the user's profile first (we launched with -NoProfile to control ordering).
if ($PROFILE -and (Test-Path $PROFILE)) { . $PROFILE }

# --- OSC 133 markers ---
$__prezzy_esc = [char]0x1b
$__prezzy_bel = [char]7

# Save original prompt so we can chain it.
$__prezzy_orig_prompt = $function:prompt

function prompt {
    $__ec = $global:LASTEXITCODE
    if ($null -eq $__ec) { $__ec = 0 }
    [Console]::Write("${__prezzy_esc}]133;D;${__ec}${__prezzy_bel}")
    [Console]::Write("${__prezzy_esc}]133;A${__prezzy_bel}")
    if ($__prezzy_orig_prompt) {
        & $__prezzy_orig_prompt
    } else {
        "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
    }
}

# Emit C marker when Enter is pressed (command about to execute).
# PSReadLine is bundled with PowerShell 5.1+ and pwsh 7+.
if (Get-Module PSReadLine -ErrorAction SilentlyContinue) {
    Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
        $e = [char]0x1b
        $b = [char]7
        [Console]::Write("${e}]133;C${b}")
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    }
}
"#;
    let mut file = tempfile::Builder::new()
        .prefix("prezzy-pwsh-")
        .suffix(".ps1")
        .tempfile()
        .context("create temp PowerShell init script")?;
    file.write_all(script.as_bytes())?;
    let path = file.into_temp_path().keep().map_err(|e| e.error)?;

    // -NoProfile: we source the profile ourselves to control ordering.
    // -NoExit: keep the shell open after init script runs.
    // -Command ". '<path>'": dot-source bypasses execution policy.
    let safe_path = path.to_string_lossy().replace('\'', "''");
    cmd.args(["-NoProfile", "-NoExit", "-Command", &format!(". '{safe_path}'")]);
    Ok(Some(path))
}

