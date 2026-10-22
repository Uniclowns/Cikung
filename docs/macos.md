# Cikung for Mac — build preparation

Cikung uses the same original orange cat head for its Windows executable, app windows/tray and Mac Finder/Dock icon. Windows ICO contains16,24,32,48,64,128 and256px images. Mac ICNS includes a1024px representation. These are conversions of the existing `assets/cikung-face.png`; no new art or third-party app assets were copied.

## Build on a Mac

The Mac package is an early platform build, not verified feature parity with Windows. This workspace was prepared on Windows. A compiled `.app` is not included in the source build kit.

1. Copy `Cikung-Mac-Build-Kit.zip` to a Mac and extract it.
2. Install stable Rust using the official Rust installer and Xcode Command Line Tools. Python3 must be available. Apple Silicon and Intel are supported build targets; the bundle declares macOS11.0 or later.
3. In the extracted folder, run `python3 scripts/build-macos.py` (or open `Build-Cikung.command`). The script installs the two Rust target standard libraries, builds the locked sources for arm64 and x86_64, then combines them with Apple's `lipo`.
4. After a successful build, extract `dist/Cikung-macOS-universal.zip` and move `Cikung.app` to Applications. Finder/Dock use the orange head stored in the bundle.

The build checks both architectures, verifies an ad-hoc code signature, and runs `--help` without opening the GUI. It packages dependency/font notices, bundle metadata and the Automation usage description for supported media apps. No Apple account or signing key is requested. Ad-hoc signing does not provide Developer ID trust or notarization: a downloaded distributed app can still be blocked by Gatekeeper. A public signed release requires the developer's Apple signing/notarization process; this preparation does not change macOS security settings.

The `Mac app package` GitHub Actions workflow performs source tests and produces the universal ZIP as an artifact when this repository is hosted on GitHub. Run it from the repository’s Actions tab using Run workflow. Native Mac packaging remains unverified until that job succeeds. The existing cross-platform checks remain in place.

## Available source paths and practical limits

- The cat notch, local date/time, separate settings, file references, tasks, focus/water timers and local notes share the Rust/egui implementation.
- Native media metadata/controls are implemented through scripting of running Spotify, Apple Music and VLC; macOS can request Automation consent. YouTube/browser discovery and artwork are not implemented in this Mac backend. Optional local VLC HTTP is also available.
- Provider chat, lyrics lookup and configured read-only service clients share platform-independent networking. Saved credentials use macOS Keychain. Live Mac account/media tests remain pending.
- Windows notification mirroring (WhatsApp), Bluetooth monitor, completion chime, desktop Claude/Codex accessibility links, native recording/dictation, tray, sign-in startup, global shortcut and fullscreen detection do not yet have Mac adapters. Some unsupported settings display an error or are disabled; they are not claimed as supported Mac features. Existing authenticated terminal-agent/phone paths are separate from desktop-app links.
- Positioning/hover/rendering on Mac hardware, camera-notch/menu-bar behavior, multi-monitor/Retina behavior, AppleScript permission flow and real playback have not been tested. The package smoke check is not a GUI acceptance test.

## Developer checks

`python3 scripts/build-macos.py --check` validates bundle/icon metadata on any host; it does not compile a Mac application. `--source-kit` writes only an explicit allowlist of source/build assets, excluding `.tools`, research, user app data and executable builds. The actual build fails on non-macOS hosts rather than creating a misleading `.app` placeholder.

Icon conversion for a future art update uses the installed Pillow development tool via `scripts/generate-app-icons.py`; Mac builders consume the committed icons and do not need Pillow.

References: [Apple bundle keys](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html), [Microsoft UpdateResource](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-updateresourcew). Windows packaging uses the native resource-update API and preserves other executable resources.

## Preparation checks on this Windows host

Both `aarch64-apple-darwin` and `x86_64-apple-darwin` passed Rust Clippy with all targets and warnings denied, using the actual Darwin standard libraries. This checks target-specific Rust types and test compilation paths; it does not link a runnable Mac executable or run Mac tests. Icon/bundle checks, source ZIP integrity/allowlist, executable LF launcher and refusal to fabricate an app on Windows also passed. Windows decoded the embedded icon natively, and all seven image payloads match the ICO. A malformed ICO was rejected before changing the test executable.
