# Cikung for Mac

Cikung uses the same original orange cat head for its Windows executable, app windows/tray and Mac Finder/Dock icon. Windows ICO contains16,24,32,48,64,128 and256px images. Mac ICNS includes a1024px representation. These are conversions of the existing `assets/cikung-face.png`; no new art or third-party app assets were copied.

## Install a release

Download `Cikung-macOS-universal.dmg` from the [GitHub Releases page](https://github.com/Uniclowns/Cikung/releases), open it and drag Cikung onto the Applications folder beside it. `Cikung-macOS-universal.zip` holds the same app: extract it and move `Cikung.app` to Applications. The universal app contains both Apple Silicon and Intel executables. You do not need the source build kit to use this package.

This is an early Mac build. The signature is ad-hoc, not Developer ID/notarized, so Gatekeeper may block a downloaded copy. No macOS security setting is changed by Cikung or its installer. Feature limits below still apply.

## Build on a Mac

The Mac package is an early platform build, not verified feature parity with Windows. This workspace was prepared on Windows. A compiled `.app` is not included in the source build kit.

1. Copy `Cikung-Mac-Build-Kit.zip` to a Mac and extract it.
2. Install stable Rust using the official Rust installer and Xcode Command Line Tools. Python3 must be available. Apple Silicon and Intel are supported build targets; the bundle declares macOS11.0 or later.
3. In the extracted folder, run `python3 scripts/build-macos.py` (or open `Build-Cikung.command`). The script installs the two Rust target standard libraries, builds the locked sources for arm64 and x86_64, then combines them with Apple's `lipo`.
4. After a successful build, open `dist/Cikung-macOS-universal.dmg` (or extract the ZIP beside it) and move `Cikung.app` to Applications. Finder/Dock use the orange head stored in the bundle.

The build checks both architectures, verifies an ad-hoc code signature, and runs `--help` without opening the GUI. It packages dependency/font notices, bundle metadata and the Automation usage description for supported media apps. No Apple account or signing key is requested. Ad-hoc signing does not provide Developer ID trust or notarization: a downloaded distributed app can still be blocked by Gatekeeper. A public signed release requires the developer's Apple signing/notarization process; this preparation does not change macOS security settings.

The `Mac app package` GitHub Actions workflow performs source tests and produces the universal DMG and ZIP as an artifact when this repository is hosted on GitHub. Run it from the repository’s Actions tab using Run workflow. A successful job verifies native compilation, both executable architectures, bundle signing integrity and the CLI smoke check; interactive GUI/media checks remain separate. The existing cross-platform checks remain in place.

## Available source paths and practical limits

- The cat notch, local date/time, separate settings, file references, tasks, focus/water timers and local notes share the Rust/egui implementation. So do the Nusa-style parts: the springy notch, cover-tinted gradient, collapsed lyric ticker, lyric flow, Figtree Bold, page carousel/indicator, gear Settings icon and Settings → Music playback options.
- Native media metadata/controls are implemented through scripting of running Spotify, Apple Music and VLC; macOS can request Automation consent. Covers (for the cover and notch tint) come from Spotify's artwork URL, fetched only from Spotify's `i.scdn.co` image server, and from Apple Music's raw artwork data. VLC has no cover. YouTube/browser discovery is not implemented in this Mac backend. Optional local VLC HTTP is also available.
- System volume buttons use AppleScript `set volume`. The AI completion chime plays the same embedded sound through the built-in `afplay`.
- The file pocket shows Finder icons, with real thumbnails for PNG/JPEG/WebP. Cards drag out to Finder, Mail or chats through an AppKit dragging session (copy or link).
- The app bundle sets `LSUIElement`: no Dock icon, like the Windows build staying off the taskbar. The cat in the menu bar opens Cikung or Settings and quits it; the cat's right-click menu and the notch's gear still work.
- Provider chat, lyrics lookup and configured read-only service clients share platform-independent networking. Saved credentials use macOS Keychain. Live Mac account/media tests remain pending.
- Windows notification mirroring (WhatsApp), Bluetooth monitor, desktop Claude/Codex accessibility links (so the Claude **App** "AI finished" notice), native recording/dictation, tray, sign-in startup, global shortcut and fullscreen detection do not yet have Mac adapters. Claude Code / Codex hook completions still notify and chime on Mac through the authenticated terminal-agent path. Some unsupported settings display an error or are disabled; they are not claimed as supported Mac features.
- Positioning/hover/rendering on Mac hardware, camera-notch/menu-bar behavior, multi-monitor/Retina behavior, AppleScript permission flow and real playback have not been tested. The package smoke check is not a GUI acceptance test.

## Developer checks

`python3 scripts/build-macos.py --check` validates bundle/icon metadata on any host; it does not compile a Mac application. `--source-kit` writes only an explicit allowlist of source/build assets, excluding `.tools`, research, user app data and executable builds. The actual build fails on non-macOS hosts rather than creating a misleading `.app` placeholder.

Icon conversion for a future art update uses the installed Pillow development tool via `scripts/generate-app-icons.py`; Mac builders consume the committed icons and do not need Pillow.

References: [Apple bundle keys](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html), [Microsoft UpdateResource](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-updateresourcew). Windows packaging uses the native resource-update API and preserves other executable resources.

## Preparation checks on this Windows host

Both `aarch64-apple-darwin` and `x86_64-apple-darwin` passed Rust Clippy with all targets, `--locked` and warnings denied, using the actual Darwin standard libraries. This was rechecked on 2026-10-23 after the Mac artwork, volume, chime and pocket adapters were added. Those adapters have not run on a Mac. This checks target-specific Rust types and test compilation paths; it does not link a runnable Mac executable or run Mac tests. Icon/bundle checks, source ZIP integrity/allowlist, executable LF launcher and refusal to fabricate an app on Windows also passed. Windows decoded the embedded icon natively, and all seven image payloads match the ICO. A malformed ICO was rejected before changing the test executable.
