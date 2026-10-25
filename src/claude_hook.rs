//! Claude Code's Stop hook runs `Cikung --agent-finished` when a turn ends. It runs in the
//! agent's own process, so a finished turn is announced even while Claude App's window is
//! paused (the screen is off). Connecting edits Claude's settings.json only when the user
//! presses Connect, after a timestamped backup beside it; disconnecting removes only the entry
//! Cikung added. Every other setting, hook and key order is kept.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

/// The flag the hook passes, and how Cikung recognises its own entry.
pub const FLAG: &str = "--agent-finished";

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).join(".claude"))
        })
        .map(|dir| dir.join("settings.json"))
}

fn ours(hook: &Value) -> bool {
    hook["command"]
        .as_str()
        .is_some_and(|command| command.contains(FLAG))
}

fn has_hook(settings: &Value) -> bool {
    settings
        .pointer("/hooks/Stop")
        .and_then(Value::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                group["hooks"]
                    .as_array()
                    .is_some_and(|hooks| hooks.iter().any(ours))
            })
        })
}

/// `settings` without Cikung's Stop entry; a Stop list left empty is removed.
fn without_hook(mut settings: Value) -> Value {
    if let Some(groups) = settings
        .pointer_mut("/hooks/Stop")
        .and_then(Value::as_array_mut)
    {
        for group in groups.iter_mut() {
            if let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                hooks.retain(|hook| !ours(hook));
            }
        }
        groups.retain(|group| {
            group["hooks"]
                .as_array()
                .is_none_or(|hooks| !hooks.is_empty())
        });
        if groups.is_empty()
            && let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut)
        {
            hooks.remove("Stop");
        }
    }
    settings
}

/// `settings` with one Cikung Stop entry running `command`, replacing an older one.
fn with_hook(settings: Value, command: &str) -> Result<Value, String> {
    let mut settings = without_hook(settings);
    let malformed = || "Claude's settings.json has an unexpected shape; it was left unchanged.";
    let root = settings.as_object_mut().ok_or_else(malformed)?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(malformed)?;
    hooks
        .entry("Stop")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(malformed)?
        .push(json!({"hooks": [{"type": "command", "command": command, "timeout": 5}]}));
    Ok(settings)
}

/// The current settings; a missing file is an empty object, unreadable JSON an error.
fn read(path: &std::path::Path) -> Result<Value, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
            "Claude's settings.json is not valid JSON, so Cikung left it unchanged.".into()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(_) => Err("Could not read Claude's settings.json.".into()),
    }
}

/// Saves `settings` after copying the current file to a timestamped backup beside it.
fn write(path: &std::path::Path, settings: &Value) -> Result<Option<PathBuf>, String> {
    let backup = if path.exists() {
        let name = format!(
            "settings.json.cikung-backup-{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let backup = path.with_file_name(name);
        fs::copy(path, &backup).map_err(|_| "Could not back up Claude's settings.json.")?;
        Some(backup)
    } else {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|_| "Could not create Claude's settings folder.")?;
        }
        None
    };
    let mut text = serde_json::to_string_pretty(settings)
        .map_err(|_| "Could not prepare Claude's settings.json.")?;
    text.push('\n');
    let temporary = path.with_file_name("settings.json.cikung-tmp");
    fs::write(&temporary, text).map_err(|_| "Could not write Claude's settings.json.")?;
    fs::rename(&temporary, path).map_err(|_| "Could not replace Claude's settings.json.")?;
    Ok(backup)
}

pub fn connected() -> bool {
    settings_path().is_some_and(|path| read(&path).is_ok_and(|settings| has_hook(&settings)))
}

/// Adds (or points to this Cikung) the Stop hook. Returns the backup, if a file existed.
pub fn connect() -> Result<Option<PathBuf>, String> {
    let path = settings_path().ok_or("Could not find your home folder.")?;
    let exe = std::env::current_exe().map_err(|_| "Could not find Cikung's own path.")?;
    // Forward slashes and quotes, like the other hooks Claude Code runs on Windows.
    let command = format!(
        "\"{}\" {FLAG}",
        exe.display().to_string().replace('\\', "/")
    );
    let settings = read(&path)?;
    let next = with_hook(settings.clone(), &command)?;
    if next == settings {
        return Ok(None);
    }
    write(&path, &next)
}

/// Removes Cikung's Stop hook. Returns the backup, if anything changed.
pub fn disconnect() -> Result<Option<PathBuf>, String> {
    let path = settings_path().ok_or("Could not find your home folder.")?;
    let settings = read(&path)?;
    let next = without_hook(settings.clone());
    if next == settings {
        return Ok(None);
    }
    write(&path, &next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connecting_keeps_other_settings_and_hooks_and_disconnecting_undoes_only_ours() {
        let original = json!({
            "env": {"A": "1"},
            "model": "opus",
            "hooks": {
                "Stop": [{"hooks": [{"type": "command", "command": "other-hook Stop", "timeout": 10}]}],
                "PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "gate"}]}]
            }
        });
        let connected = with_hook(original.clone(), "\"C:/Cikung.exe\" --agent-finished").unwrap();
        assert!(has_hook(&connected));
        let stop = connected
            .pointer("/hooks/Stop")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0], original.pointer("/hooks/Stop/0").cloned().unwrap());
        // Key order is preserved, so the file reads the same apart from the new entry.
        let keys: Vec<_> = connected.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["env", "model", "hooks"]);
        // Connecting again replaces Cikung's entry instead of adding a second one.
        let again = with_hook(connected.clone(), "\"D:/Cikung.exe\" --agent-finished").unwrap();
        assert_eq!(
            again
                .pointer("/hooks/Stop")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(without_hook(again), original);
    }

    #[test]
    fn an_empty_file_gains_and_loses_only_the_hook() {
        let connected = with_hook(json!({}), "cikung --agent-finished").unwrap();
        assert!(has_hook(&connected));
        assert_eq!(without_hook(connected), json!({"hooks": {}}));
        assert!(with_hook(json!([]), "x").is_err());
        assert!(!has_hook(&json!({"hooks": {"Stop": "oops"}})));
    }
}
