//! Session-scoped integrations; never edits Claude/OMP/Codex user configuration.
pub mod codex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Agent {
    #[default]
    Claude,
    Omp,
    Codex,
}
impl Agent {
    pub const ALL: [Self; 3] = [Self::Claude, Self::Omp, Self::Codex];
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Omp => "Oh My Pi",
            Self::Codex => "Codex",
        }
    }
    pub fn companion_label(self) -> &'static str {
        match self {
            Self::Claude => crate::desktop_apps::Target::Claude.label(),
            Self::Codex => crate::desktop_apps::Target::Codex.label(),
            Self::Omp => self.label(),
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Claude => 0,
            Self::Omp => 1,
            Self::Codex => 2,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub selected: Agent,
    pub executables: [String; 3],
    pub node: String,
    pub project: String,
    pub omp_gate: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            selected: Agent::Claude,
            executables: [find("claude"), find("omp"), find("codex")],
            node: find("node"),
            project: String::new(),
            omp_gate: false,
        }
    }
}
fn find(name: &str) -> String {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let home = PathBuf::from(home);
        dirs.extend([
            home.join(".local/bin"),
            home.join(".bun/bin"),
            home.join("AppData/Roaming/npm"),
        ]);
    }
    for dir in dirs {
        for suffix in if cfg!(windows) {
            vec![".exe", ".ps1", ".cmd", ""]
        } else {
            vec![""]
        } {
            let p = dir.join(format!("{name}{suffix}"));
            if p.is_file() {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    String::new()
}

pub struct Manager {
    running: Vec<Running>,
}
struct Running {
    child: Child,
    directory: PathBuf,
}
impl Manager {
    pub fn new() -> Self {
        Self {
            running: Vec::new(),
        }
    }
    pub fn poll(&mut self) {
        self.running.retain_mut(|run| match run.child.try_wait() {
            Ok(Some(_)) => {
                cleanup(&run.directory);
                false
            }
            _ => true,
        });
    }
    pub fn launch(
        &mut self,
        settings: &Settings,
        endpoint: &str,
        token: &str,
    ) -> Result<(), String> {
        let executable = checked(
            &settings.executables[settings.selected.index()],
            "agent executable",
        )?;
        let node = if needs_node(settings.selected, &executable) {
            checked(&settings.node, "Node.js executable")?
        } else {
            PathBuf::new()
        };
        let project = Path::new(&settings.project)
            .canonicalize()
            .map_err(|_| "Choose an existing project folder first.")?;
        if !project.is_dir() {
            return Err("Choose a project folder, not a file.".into());
        }
        validate_connection(endpoint, token)?;
        let root = crate::instance::data_dir().join("agent-launches");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let directory = root.join(nonce);
        private_directory(&directory)?;
        let result = (|| {
            let args = prepare(&directory, settings.selected, &node, endpoint, token)?;
            #[cfg(windows)]
            let (launch_executable, launch_args) =
                native_launch_target(settings.selected, &executable, &node, args.clone())?;
            #[cfg(not(windows))]
            let (launch_executable, launch_args) = (executable.clone(), args.clone());
            let profile = directory.join("launch.json");
            fs::write(&profile,serde_json::to_vec(&json!({"executable":launch_executable,"arguments":launch_args,"project":project,"agent":settings.selected.label()})).unwrap()).map_err(|e|e.to_string())?;
            let wrapper = directory.join("launch-agent.ps1");
            fs::write(&wrapper, include_str!("../../scripts/launch-agent.ps1"))
                .map_err(|e| e.to_string())?;
            #[cfg(windows)]
            let mut command = {
                use std::os::windows::process::CommandExt;
                let shell = std::env::var_os("SystemRoot")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| "C:\\Windows".into())
                    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
                let mut c = Command::new(shell);
                c.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-NoExit",
                    "-File",
                ])
                .arg(wrapper)
                .arg("-Profile")
                .arg(profile)
                .creation_flags(0x10);
                c
            };
            #[cfg(not(windows))]
            let mut command = {
                let mut c = Command::new("x-terminal-emulator");
                c.arg("-e").arg(&executable).args(&args);
                c
            };
            command
                .current_dir(&project)
                .env("CIKUNG_ENDPOINT", endpoint)
                .env("CIKUNG_TOKEN", token)
                .env("CIKUNG_PROJECT", &project)
                .env(
                    "CIKUNG_OMP_APPROVALS",
                    if settings.omp_gate { "1" } else { "0" },
                );
            let child = command
                .spawn()
                .map_err(|e| format!("Could not open the connected terminal: {e}"))?;
            Ok(child)
        })();
        match result {
            Ok(child) => {
                self.running.push(Running { child, directory });
                Ok(())
            }
            Err(e) => {
                cleanup(&directory);
                Err(e)
            }
        }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        for run in &self.running {
            cleanup(&run.directory);
        }
    }
}
fn checked(value: &str, label: &str) -> Result<PathBuf, String> {
    let p = Path::new(value)
        .canonicalize()
        .map_err(|_| format!("Choose a valid {label} in Connection settings."))?;
    if !p.is_file() {
        return Err(format!("The {label} is not a file."));
    }
    Ok(p)
}

pub fn needs_node(agent: Agent, executable: &Path) -> bool {
    if agent != Agent::Omp {
        return true;
    }
    // Native OMP embeds its JavaScript runtime. Only a Windows npm shim needs
    // an external Node executable for the lossless native launch path.
    cfg!(windows)
        && !executable
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
}

/// Native ProcessStartInfo preserves argv. Standard npm shell shims do not:
/// Windows PowerShell 5.1 can strip embedded TOML quotes again inside the shim.
/// Resolve only the selected agent's known npm package and its declared entry.
#[cfg(any(windows, test))]
fn native_launch_target(
    agent: Agent,
    executable: &Path,
    node: &Path,
    arguments: Vec<String>,
) -> Result<(PathBuf, Vec<String>), String> {
    let extension = executable
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if extension.eq_ignore_ascii_case("exe") {
        return Ok((executable.into(), arguments));
    }
    if !extension.eq_ignore_ascii_case("ps1") && !extension.eq_ignore_ascii_case("cmd") {
        return Err("Choose the agent's native .exe or standard npm .ps1/.cmd launcher.".into());
    }
    let (name, package) = match agent {
        Agent::Claude => ("claude", "@anthropic-ai/claude-code"),
        Agent::Omp => ("omp", "@oh-my-pi/pi-coding-agent"),
        Agent::Codex => ("codex", "@openai/codex"),
    };
    if executable.file_stem().and_then(|s| s.to_str()) != Some(name) {
        return Err("Choose the standard agent launcher or its native executable.".into());
    }
    let package_root = executable.parent().ok_or("Agent launcher has no parent folder")?
        .join("node_modules").join(package).canonicalize()
        .map_err(|_| "The selected shell launcher is not a standard npm installation. Choose its native .exe instead.")?;
    let metadata_path = package_root.join("package.json");
    if fs::metadata(&metadata_path)
        .map_err(|e| e.to_string())?
        .len()
        > 128 * 1024
    {
        return Err("Agent package metadata is too large.".into());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(metadata_path).map_err(|e| e.to_string())?)
            .map_err(|_| "Could not read the agent's npm entry point.")?;
    if metadata["name"].as_str() != Some(package) {
        return Err("Agent npm package identity does not match the selected launcher.".into());
    }
    let declared = metadata["bin"]
        .as_str()
        .or_else(|| metadata["bin"][name].as_str())
        .ok_or("The agent package does not declare the expected CLI entry point.")?;
    let entry = package_root
        .join(declared)
        .canonicalize()
        .map_err(|_| "The agent's npm entry point is missing.")?;
    if !entry.is_file()
        || !entry.starts_with(&package_root)
        || !matches!(
            entry.extension().and_then(|s| s.to_str()),
            Some("js" | "mjs" | "cjs")
        )
    {
        return Err("The agent's npm entry point is not a contained JavaScript file.".into());
    }
    let mut native_arguments = vec![
        entry
            .to_str()
            .ok_or("Agent entry paths must use Unicode.")?
            .into(),
    ];
    native_arguments.extend(arguments);
    Ok((node.into(), native_arguments))
}
fn validate_connection(endpoint: &str, token: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(endpoint).map_err(|_| "Invalid local bridge address.")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Agent bridge must be the local Cikung loopback address.".into());
    }
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid agent connection token.".into());
    }
    Ok(())
}
fn prepare(
    directory: &Path,
    agent: Agent,
    node: &Path,
    endpoint: &str,
    token: &str,
) -> Result<Vec<String>, String> {
    match agent {
        Agent::Claude => {
            let adapter = directory.join("cikung-claude.cjs");
            fs::write(&adapter, include_str!("../../adapters/cikung-claude.cjs"))
                .map_err(|e| e.to_string())?;
            let settings = claude_settings(node, &adapter)?;
            let path = directory.join("claude-settings.json");
            fs::write(&path, serde_json::to_vec_pretty(&settings).unwrap())
                .map_err(|e| e.to_string())?;
            Ok(vec![
                "--settings".into(),
                path.to_string_lossy().into_owned(),
            ])
        }
        Agent::Omp => {
            let path = directory.join("cikung-omp.ts");
            fs::write(&path, include_str!("../../adapters/cikung-omp.ts"))
                .map_err(|e| e.to_string())?;
            Ok(vec![
                "--extension".into(),
                path.to_string_lossy().into_owned(),
            ])
        }
        Agent::Codex => {
            let adapter = directory.join("cikung-codex-hook.cjs");
            fs::write(&adapter, codex::HOOK_SOURCE).map_err(|e| e.to_string())?;
            let connection = directory.join("connection.json");
            fs::write(
                &connection,
                serde_json::to_vec(&json!({"endpoint":endpoint,"token":token})).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            codex::launch_args(node, &adapter, Some(&connection))
        }
    }
}
fn claude_settings(node: &Path, adapter: &Path) -> Result<serde_json::Value, String> {
    fn quote(p: &Path) -> Result<String, String> {
        let s = p.to_str().ok_or("Hook paths must use Unicode.")?;
        if s.chars().any(|c| c.is_control()) {
            return Err("Hook path contains a control character.".into());
        }
        Ok(if cfg!(windows) {
            format!("'{}'", s.replace('\'', "''"))
        } else {
            format!("'{}'", s.replace('\'', "'\\''"))
        })
    }
    let mut hooks = serde_json::Map::new();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "PermissionRequest",
        "Notification",
        "Stop",
        "SessionEnd",
    ] {
        let command = format!(
            "{}{} {} --event '{event}'",
            if cfg!(windows) { "& " } else { "" },
            quote(node)?,
            quote(adapter)?
        );
        let mut hook = json!({"type":"command","command":command,"timeout":if event=="PermissionRequest"{115}else{5}});
        if cfg!(windows) {
            hook["shell"] = json!("powershell");
        }
        hooks.insert(event.into(), json!([{"hooks":[hook]}]));
    }
    Ok(json!({"hooks":hooks}))
}
fn cleanup(directory: &Path) {
    for file in [
        "connection.json",
        "launch.json",
        "launch-agent.ps1",
        "claude-settings.json",
        "cikung-claude.cjs",
        "cikung-omp.ts",
        "cikung-codex-hook.cjs",
    ] {
        let _ = fs::remove_file(directory.join(file));
    }
    let _ = fs::remove_dir(directory);
}

#[cfg(windows)]
fn private_directory(path: &Path) -> Result<(), String> {
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::{
                Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
                PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
            },
            Storage::FileSystem::CreateDirectoryW,
        },
        core::{HSTRING, w},
    };
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;OW)"),
            1,
            &mut descriptor,
            None,
        )
    }
    .map_err(|e| format!("Could not protect the agent connection: {e}"))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let result = unsafe { CreateDirectoryW(&HSTRING::from(path.as_os_str()), Some(&attributes)) }
        .map_err(|e| format!("Could not create the private connection folder: {e}"));
    unsafe {
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
    }
    result
}
#[cfg(not(windows))]
fn private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_directory() -> PathBuf {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).unwrap();
        let nonce: String = nonce.iter().map(|n| format!("{n:02x}")).collect();
        let directory = std::env::temp_dir().join(format!("cikung-launch-fixture-{nonce}"));
        fs::create_dir(&directory).unwrap();
        directory
    }

    #[test]
    fn standard_codex_npm_launcher_uses_its_declared_entry_and_preserves_argv() {
        let directory = fixture_directory();
        let package = directory.join("node_modules/@openai/codex");
        fs::create_dir_all(package.join("bin")).unwrap();
        fs::write(
            package.join("package.json"),
            br#"{"name":"@openai/codex","bin":{"codex":"bin/codex.js"}}"#,
        )
        .unwrap();
        fs::write(package.join("bin/codex.js"), b"// fixture, never executed").unwrap();
        let args = vec![
            "-c".into(),
            "hooks.Stop=[{hooks=[{type=\"command\"}]}]".into(),
        ];
        let (program, resolved) = native_launch_target(
            Agent::Codex,
            &directory.join("codex.ps1"),
            Path::new("node.exe"),
            args.clone(),
        )
        .unwrap();
        assert_eq!(program, Path::new("node.exe"));
        assert_eq!(&resolved[1..], args.as_slice());
        assert_eq!(
            Path::new(&resolved[0]),
            package.join("bin/codex.js").canonicalize().unwrap()
        );
        fs::write(
            package.join("package.json"),
            br#"{"name":"@openai/codex","bin":{"codex":"../../../outside.js"}}"#,
        )
        .unwrap();
        fs::write(directory.join("outside.js"), b"// fixture").unwrap();
        assert!(
            native_launch_target(
                Agent::Codex,
                &directory.join("codex.ps1"),
                Path::new("node.exe"),
                args
            )
            .is_err()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn windows_terminal_preserves_native_argument_data() {
        use std::os::windows::process::CommandExt;
        let node = find("node");
        assert!(
            !node.is_empty(),
            "The Windows launch fixture requires Node.js"
        );
        let directory = fixture_directory();
        let echo = directory.join("echo arguments.cjs");
        let output = directory.join("arguments.json");
        fs::write(&echo, b"require('node:fs').writeFileSync(process.argv[2],JSON.stringify(process.argv.slice(3)))").unwrap();
        let expected = vec![
            "",
            "with spaces",
            r#"hooks.Stop=[{hooks=[{type="command",command="node path",timeout=3}]}]"#,
            "C:\\trailing\\",
            "猫 ภาษาไทย",
            "$(& data); `quoted`",
            "a\"b\\\\c",
        ];
        let mut arguments = vec![
            echo.to_str().unwrap().to_owned(),
            output.to_str().unwrap().to_owned(),
        ];
        arguments.extend(expected.iter().map(|s| (*s).to_owned()));
        let profile = directory.join("launch.json");
        fs::write(&profile, serde_json::to_vec(&json!({"executable":node,"arguments":arguments,"project":directory,"agent":"Fixture"})).unwrap()).unwrap();
        let wrapper = directory.join("launch-agent.ps1");
        fs::write(&wrapper, include_str!("../../scripts/launch-agent.ps1")).unwrap();
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let result = Command::new(shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(wrapper)
            .arg("-Profile")
            .arg(profile)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let actual: Vec<String> = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn launcher_settings_do_not_weaken_permissions_or_contain_secrets() {
        let value = claude_settings(
            Path::new("C:/Node's folder/node.exe"),
            Path::new("C:/my adapter.cjs"),
        )
        .unwrap();
        let text = value.to_string();
        assert!(text.contains("PermissionRequest"));
        assert!(!text.contains("CIKUNG_TOKEN"));
        assert!(!text.contains("bypass"));
        let hook = &value["hooks"]["PermissionRequest"][0]["hooks"][0];
        let command = hook["command"].as_str().unwrap();
        let escaped = if cfg!(windows) {
            "Node''s"
        } else {
            "Node'\\''s"
        };
        assert!(command.contains(escaped));
        assert_eq!(
            hook.get("shell").and_then(|s| s.as_str()),
            if cfg!(windows) {
                Some("powershell")
            } else {
                None
            }
        );
        assert_eq!(
            value["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            115
        );
    }
    #[test]
    fn launch_connection_is_local_and_has_a_real_token() {
        assert!(validate_connection("http://127.0.0.1:1234", &"a".repeat(64)).is_ok());
        for bad in [
            "https://example.com",
            "http://localhost:1234",
            "http://user@127.0.0.1:1234",
            "http://127.0.0.1:1234/?x=1",
        ] {
            assert!(validate_connection(bad, &"a".repeat(64)).is_err());
        }
    }

    #[test]
    fn native_omp_does_not_require_an_external_node_runtime() {
        assert!(!needs_node(Agent::Omp, Path::new("omp.exe")));
        assert!(needs_node(Agent::Claude, Path::new("claude.exe")));
        assert!(needs_node(Agent::Codex, Path::new("codex.exe")));
        if cfg!(windows) {
            assert!(needs_node(Agent::Omp, Path::new("omp.ps1")));
        }
    }
    #[test]
    fn private_launch_folder_can_be_written_and_removed_by_its_owner() {
        let p = std::env::temp_dir().join(format!(
            "cikung-private-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        private_directory(&p).unwrap();
        fs::write(p.join("connection.json"), b"test-only").unwrap();
        assert_eq!(fs::read(p.join("connection.json")).unwrap(), b"test-only");
        cleanup(&p);
        assert!(!p.exists());
    }
}
