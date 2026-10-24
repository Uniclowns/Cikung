//! Windows Setup without an installer toolkit. `Cikung-Setup.exe` (any file name with
//! "setup") or `--install` copies this exe to %LOCALAPPDATA%\Programs\Cikung, adds a
//! Start menu shortcut and an Installed apps entry, then starts the installed copy, which
//! turns on sign-in startup itself. `--uninstall`, run by Installed apps, removes them.
//! Everything is per user: no administrator rights.
use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    time::Duration,
};
use windows::{
    Win32::UI::WindowsAndMessaging::{
        IDOK, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_OKCANCEL,
        MB_SETFOREGROUND, MESSAGEBOX_STYLE, MessageBoxW,
    },
    core::{HSTRING, w},
};

const UNINSTALL_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Cikung";
const NO_WINDOW: u32 = 0x0800_0000;

/// Runs Setup or the uninstaller when asked; false means start Cikung as usual.
pub fn run(args: &[String]) -> bool {
    let remove = args.iter().any(|a| a == "--uninstall");
    if !remove && !args.iter().any(|a| a == "--install") && !launched_as_setup() {
        return false;
    }
    let result = if remove { uninstall() } else { install() };
    if let Err(error) = result {
        message(&error, MB_ICONERROR);
    }
    true
}

fn launched_as_setup() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().to_lowercase())
        })
        .is_some_and(|name| name.contains("setup"))
}

fn install_folder() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|base| PathBuf::from(base).join("Programs").join("Cikung"))
        .ok_or_else(|| "Your local app data folder is unavailable.".into())
}

fn start_menu_shortcut() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|base| {
        PathBuf::from(base).join("Microsoft\\Windows\\Start Menu\\Programs\\Cikung.lnk")
    })
}

fn install() -> Result<(), String> {
    let folder = install_folder()?;
    let question = format!(
        "Install Cikung {} for your Windows account?\n\nIt goes to {}, gets a Start menu shortcut and starts when you sign in. Remove it any time in Settings → Apps → Installed apps.",
        env!("CARGO_PKG_VERSION"),
        folder.display()
    );
    if !ask(&question) {
        return Ok(());
    }
    stop_running()?;
    let target = folder.join("Cikung.exe");
    let source = std::env::current_exe().map_err(|e| format!("Could not locate Setup: {e}"))?;
    let program = std::fs::read(&source).map_err(|e| format!("Could not read Setup: {e}"))?;
    if !source.as_os_str().eq_ignore_ascii_case(target.as_os_str()) {
        std::fs::create_dir_all(&folder)
            .map_err(|e| format!("Could not create {}: {e}", folder.display()))?;
        // A new file, as any installer writes, not a copy of the download: a copy keeps the
        // downloaded-file warning for every Start menu launch.
        std::fs::write(&target, &program).map_err(|e| format!("Could not install Cikung: {e}"))?;
    }
    if let Some(link) = start_menu_shortcut() {
        shortcut(&link, &target)
            .map_err(|e| format!("Could not add the Start menu shortcut: {e}"))?;
    }
    register(&target, &folder, program.len())?;
    std::process::Command::new(&target)
        .current_dir(&folder)
        .spawn()
        .map_err(|e| format!("Cikung is installed but did not start: {e}"))?;
    message(
        "Cikung is installed and running at the top of your screen. Open it from the Start menu; it also starts when you sign in.",
        MB_ICONINFORMATION,
    );
    Ok(())
}

fn uninstall() -> Result<(), String> {
    if !ask(
        "Remove Cikung from this PC?\n\nYour settings, notes and tasks stay in your Windows profile.",
    ) {
        return Ok(());
    }
    stop_running()?;
    let _ = crate::shell::set_autostart(false);
    if let Some(link) = start_menu_shortcut() {
        let _ = std::fs::remove_file(link);
    }
    let _ = crate::shell::registry_command(&["delete", UNINSTALL_KEY, "/f"]);
    message("Cikung was removed.", MB_ICONINFORMATION);
    let folder = install_folder()?;
    let path = folder.to_string_lossy();
    // Windows can't delete a running program (this may be the installed exe), so a hidden
    // cmd removes the folder after this process exits. ponytail: a "%" or quote in the path
    // would need cmd escaping; that folder is left for the user to delete.
    if folder.exists() && !path.contains(['%', '"']) {
        let _ = std::process::Command::new(
            crate::shell::registry_executable().with_file_name("cmd.exe"),
        )
        .raw_arg(format!(
            "/d /c ping -n 3 127.0.0.1 >nul & rd /s /q \"{path}\""
        ))
        .current_dir(std::env::temp_dir())
        .creation_flags(NO_WINDOW)
        .spawn();
    }
    Ok(())
}

/// Closes a running Cikung, politely first: it would keep the old version running and
/// the installed exe busy.
fn stop_running() -> Result<(), String> {
    let others = format!("PID ne {}", std::process::id());
    for force in [false, true] {
        if !running() {
            return Ok(());
        }
        let mut args = vec!["/IM", "Cikung.exe", "/FI", others.as_str()];
        if force {
            args.push("/F");
        }
        let _ = std::process::Command::new(
            crate::shell::registry_executable().with_file_name("taskkill.exe"),
        )
        .args(&args)
        .creation_flags(NO_WINDOW)
        .output();
        for _ in 0..30 {
            if !running() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Err("Cikung is still running. Quit it (right-click the cat → Quit) and try again.".into())
}

/// Another Cikung holds the one-window guard.
fn running() -> bool {
    matches!(crate::instance::Instance::acquire(), Ok(None))
}

fn register(target: &Path, folder: &Path, size: usize) -> Result<(), String> {
    let program = target.to_string_lossy();
    let values = [
        ("DisplayName", "REG_SZ", "Cikung".to_owned()),
        (
            "DisplayVersion",
            "REG_SZ",
            env!("CARGO_PKG_VERSION").to_owned(),
        ),
        ("Publisher", "REG_SZ", "Uniclowns".to_owned()),
        ("DisplayIcon", "REG_SZ", program.to_string()),
        (
            "InstallLocation",
            "REG_SZ",
            folder.to_string_lossy().into_owned(),
        ),
        (
            "UninstallString",
            "REG_SZ",
            format!("\"{program}\" --uninstall"),
        ),
        ("NoModify", "REG_DWORD", "1".to_owned()),
        ("NoRepair", "REG_DWORD", "1".to_owned()),
        ("EstimatedSize", "REG_DWORD", (size / 1024).to_string()),
    ];
    for (name, kind, data) in values {
        let added = crate::shell::registry_command(&[
            "add",
            UNINSTALL_KEY,
            "/v",
            name,
            "/t",
            kind,
            "/d",
            data.as_str(),
            "/f",
        ])
        .is_ok_and(|output| output.status.success());
        if !added {
            return Err("Windows did not accept the Installed apps entry for Cikung.".into());
        }
    }
    Ok(())
}

fn shortcut(link: &Path, target: &Path) -> windows::core::Result<()> {
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                IPersistFile,
            },
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::Interface,
    };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        shell_link.SetPath(&HSTRING::from(target.as_os_str()))?;
        if let Some(folder) = target.parent() {
            shell_link.SetWorkingDirectory(&HSTRING::from(folder.as_os_str()))?;
        }
        shell_link.SetDescription(w!("A cat in a notch for your music, tools and AI agents"))?;
        shell_link
            .cast::<IPersistFile>()?
            .Save(&HSTRING::from(link.as_os_str()), true)
    }
}

fn ask(text: &str) -> bool {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            w!("Cikung Setup"),
            MB_OKCANCEL | MB_ICONQUESTION | MB_SETFOREGROUND,
        ) == IDOK
    }
}

fn message(text: &str, icon: MESSAGEBOX_STYLE) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            w!("Cikung Setup"),
            MB_OK | icon | MB_SETFOREGROUND,
        );
    }
}
