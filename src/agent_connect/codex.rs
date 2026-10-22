//! Codex CLI lifecycle hooks, checked against codex-cli 0.157.1.
//! These invocation-only overrides do not edit a user's Codex configuration.
use std::path::Path;

pub const HOOK_SOURCE: &str = include_str!("../../scripts/cikung-codex-hook.cjs");
pub const REVIEW_NOTICE: &str = "In Codex, open /hooks and review/trust the Cikung hooks. Existing Codex approval and sandbox policies still apply.";

/// Build separate argv values for `codex`; never join this result into a shell command.
/// A connection file avoids depending on a host's filtered shell environment.
/// Its JSON is {"endpoint":"http://127.0.0.1:PORT","token":"..."}.
pub fn launch_args(
    node: &Path,
    hook_script: &Path,
    connection: Option<&Path>,
) -> Result<Vec<String>, String> {
    let mut command = format!("{} {}", shell_path(node)?, shell_path(hook_script)?);
    if let Some(connection) = connection {
        command.push_str(" --connection ");
        command.push_str(&shell_path(connection)?);
    }
    let mut args = Vec::new();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
        "PreCompact",
        "PostCompact",
        "Stop",
        "Interrupt",
        "SessionEnd",
    ] {
        let hook_command = format!("{command} --event {event}");
        // JSON string escaping is also valid for these TOML basic strings.
        let encoded = serde_json::to_string(&hook_command).map_err(|e| e.to_string())?;
        let timeout = if event == "PermissionRequest" { 115 } else { 3 };
        args.push("-c".into());
        args.push(format!(
            "hooks.{event}=[{{hooks=[{{type=\"command\",command={encoded},timeout={timeout}}}]}}]"
        ));
    }
    Ok(args)
}

/// Hooks execute via the host's command shell; keep executable/script paths literal.
/// Windows command expansion is rejected rather than relying on fragile cmd escaping.
fn shell_path(path: &Path) -> Result<String, String> {
    let value = path
        .to_str()
        .ok_or("Codex hook paths must use Unicode characters")?;
    if value.is_empty() || value.chars().any(|c| c.is_control()) {
        return Err("Codex hook path is empty or contains a control character".into());
    }
    #[cfg(windows)]
    {
        if value.contains(['"', '%', '!']) {
            return Err("Move the Codex hook and Node executable to paths without quotes, percent signs, or exclamation marks".into());
        }
        Ok(format!("\"{value}\""))
    }
    #[cfg(not(windows))]
    {
        Ok(format!("'{}'", value.replace('\'', "'\\''")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invocation_adds_real_lifecycle_events_without_policy_overrides() {
        let args = launch_args(
            Path::new("node"),
            Path::new("hook with spaces.cjs"),
            Some(Path::new("connection.json")),
        )
        .unwrap();
        assert_eq!(args.len(), 20);
        assert!(
            args.iter()
                .any(|s| s.starts_with("hooks.PermissionRequest=") && s.contains("timeout=115"))
        );
        assert!(args.iter().any(|s| s.contains("--event Stop")));
        assert!(!args.iter().any(|s| s.contains("bypass")
            || s.contains("approval_policy")
            || s.contains("sandbox")
            || s.contains("features.hooks")));
    }

    #[test]
    fn empty_and_multiline_paths_are_rejected() {
        assert!(launch_args(Path::new(""), Path::new("hook"), None).is_err());
        assert!(launch_args(Path::new("node\ncommand"), Path::new("hook"), None).is_err());
    }

    #[test]
    #[cfg(windows)]
    fn cmd_expansion_in_paths_is_rejected() {
        for path in ["node%PATH%.exe", "node!name!.exe", "node\".exe"] {
            assert!(launch_args(Path::new(path), Path::new("hook"), None).is_err());
        }
    }
}
