# Cikung 0.4 — one app with multiple cat types

Date: 2026-10-19, Asia/Bangkok. Windows x64 at 125% scale.

- Corrected the previous interpretation: selecting a feature or cat stays inside one application and one notch. Six generated breeds and per-feature/per-agent assignments replace the companion-process architecture. Files, Tasks, Notes, Agents and Phone share one state.
- 74 Rust tests passed, including actual image transparency, automatic/manual cat selection, preference persistence, 14-panel layout, one-instance locking, protected agent launching, and five isolated profile-migration cases. Migration tests cover conflicting IDs, field ownership, exactly-once backup/marker behavior, malformed input and bootstrap without a workspace profile.
- 19 Node adapter tests and 8 Bun adapter tests passed against local fixtures. Total automated tests: 101. No signed-in model session or paid prompt was launched.
- Formatting and Clippy with warnings denied passed. Original v0.3 profiles are retained; migration uses an atomic shared-profile replacement and an original-profile backup.
- Runtime reference evidence is scoped in coucou-live-reference.md. REA bound the installed CouCou binary; its native UI observer was unavailable on Windows, so Windows UI Automation and a selected-window capture supplied the limited runtime observations. No global CouCou settings or accounts were modified.
- Own-window previews: cikung-v4-overview.png and cikung-v4-cats.png. Native hover, singleton relaunch and package verification are recorded in single-notch-hover-smoke.json, single-instance-smoke.json and dist/release-manifest.json.
- Native macOS/Linux validation and physical-phone/live-agent account tests remain pending.

# Cikung 0.3 agent and companion update

Date: 2026-10-19, Asia/Bangkok. Windows x64, 1920 × 1080 at 125% scale.

- 70 Rust tests passed. Added actual PowerShell 5.1 argument-preservation fixtures, invocation-only hook configuration, protected connection directory checks, authenticated Phone-to-Agents relay (including rejection of adapter self-approval), stale permission removal, typed companion handoffs, geometry and small/expanded UI layout checks.
- 19 Node tests passed for the actual Claude and Codex adapter scripts. Eight Bun tests (38 assertions) passed for the actual Oh My Pi extension. All used local fixtures; no real agent prompt, login flow or paid API call was performed.
- Formatting and Clippy with warnings denied passed. The optimized Windows build passed.
- Release native hover checks passed: stable host, centered shape, brief-pass rejection, leave/re-entry, automatic collapse, explicit Escape close suppression and click-through outside the notch. Opening was sampled at 18 distinct widths; this does not establish an FPS guarantee.
- Release fleet checks passed with three concurrent native windows: distinct small cats, aligned top edge, even spacing, independent hover expansion and return to the original slot. At 125% scale each collapsed cat was 165 × 50 physical pixels. See fleet-smoke.json and hover-smoke.json.
- Own-window captures: cikung-v3-agents.png and cikung-v3-music.png. Agent executable detection is real, but preview mode intentionally disables connecting. Diagnostics use disposable storage, preserving normal profiles.
- Phone relay tests use authenticated loopback fixtures. Physical-phone testing, real signed-in Claude/OMP/Codex coding sessions, Codex hook trust, and native macOS/Linux fleets remain unvalidated.
- Distribution verification and SHA-256 values are recorded in dist/release-manifest.json.

# Cikung 0.2.1 hover update

Date: 2026-10-18, Asia/Bangkok. Windows x64 at 125% display scale.

- All 53 tests passed. New regressions cover bounded, centered motion; reversing mid-transition; reduced-motion behavior; hover dwell; brief passes; leave/re-entry; explicit-close suppression; popup/drag/pin holds; and Playback options remaining visible in the compact Music panel.
- Formatting and Clippy with warnings denied passed. The optimized 0.2.1 release build passed.
- The final release executable passed `scripts/hover-smoke.ps1` against real Windows cursor input and native window regions. All checks passed: fixed host, centered shape, brief-pass rejection, re-entry cancellation, auto-close, explicit Escape close remaining closed, and click-through outside the shape. Opening was sampled at 18 distinct intermediate widths; this is a sampled shape progression, not a universal FPS guarantee. Full samples are in `hover-smoke.json`.
- Final native smoke verified top-center host placement, the shaped input region, tray, global shortcut, and local agent bridge. Phone, recording, and dictation remained off. `hover-native-audit.json` records the host rectangle and the smaller visible-notch rectangle separately.
- Final native screenshots include `cikung-hover-music.png`, `cikung-hover-more.png`, `cikung-hover-focus.png`, `cikung-hover-settings.png`, and `cikung-hover-collapsed.png`. Demo music is explicitly labeled Preview.
- The release ZIP was extracted into a fresh folder. The extracted executable's SHA-256 matched the packaged executable, and its real `--probe` returned an empty session list successfully. The manifest and artifact hashes are in `dist/release-manifest.json`.

Reference evidence, implementation choices, and platform limits are documented in `hover-polish.md`. The notes below describe earlier releases.

# Cikung 0.2 validation

Host: Windows x64, 1920 × 1080 at 125% scaling. Date: 2026-10-18, Asia/Bangkok.

## Checks completed for 0.2

- All 47 tests passed. Coverage includes native media timeline/seek validation, mocked VLC requests, provider/service response parsing and secret handling, real TCP authentication/pairing/approval/revocation, local productivity state, synthetic audio conversion, asynchronous request races, and horizontal layout of all 13 panels.
- The included PowerShell Claude Code adapter was executed against a real local bridge fixture: it published a permission preview, waited for an explicit decision, and returned the expected Claude Code `deny` response. No agent tool or service action was executed.
- `cargo clippy --all-targets --locked -- -D warnings` passed on the Windows GNU target. Rust formatting was applied and checked.
- The actual Windows media fixture was rerun for 0.2. The backend discovered its real title, artist, unique album marker, and Playing state; real Pause and Resume worked; unsupported Previous/Next returned errors. The source was silent and muted, and its temporary WAV was removed after completion.
- `--native-smoke` opened the native app and measured its actual window rectangle. At 125% scale, the collapsed 252 × 40 logical notch occupied `[802, 0, 1117, 50]` physical pixels, exactly matching the top-center target. Tray, global shortcut, and loopback agent server initialized. Phone access, microphone/system recording, and dictation were all off. The check did not change startup settings or saved preferences. Results: `native-v2-audit.json`.
- Native window screenshots cover the collapsed cat, Music, Focus, Phone, Style, and Settings pages. `cikung-v2-music.png` is explicitly labeled demo data. Native microphone capture and Windows speech recognition were not started during development.
- The optimized 0.2 release build passed. Both collapsed and expanded release windows matched their measured top-center targets; the expanded rectangle was `[597, 0, 1322, 550]`. The ZIP was extracted into a separate fresh directory, the extracted executable's SHA-256 matched the packaged executable, and `--probe` exited successfully with an empty real-session list after the fixture closed. The approximately 10.4 MiB ZIP includes dependency notices and all seven integration/setup guides. Artifact hashes are in `dist/release-manifest.json`.

## Remaining validation

- Live cloud accounts need the user's keys, model access, and read permissions. Mocked request tests do not establish that an account is connected.
- A physical phone on the user's Wi-Fi has not been paired here. TCP tests validate the authentication/control protocol on loopback; trusted-LAN HTTP, native mobile app, push, and widget limits are documented in `agent-phone-link.md`.
- Actual Spotify, YouTube/browser, and VLC installations have not been exercised. Windows native API and mocked VLC fallback checks do not guarantee every player exposes complete metadata or controls.
- Native macOS/Linux builds and runtime behavior remain unverified on this Windows host. The three-platform CI workflow has not been dispatched. Non-Windows native shell, recording, and speech adapters remain open work.
- Multiple physical monitors, permission dialogs, real microphones, speakers, and human pointer interactions require device testing. Geometry tests cover negative display origins and DPI; the native smoke check covers this host's primary display.
- This portable build is unsigned. `feature-coverage.md` lists the remaining upstream scope; 0.2 is not complete CouCou parity.

# Historical: Cikung 0.1 validation

The notes below describe the earlier full-body MVP. Their feature limits do not describe the expanded 0.2 release above.

Host: Windows x64. Date: 2026-10-18, Asia/Bangkok.

## Verified

- Windows GNU build with Rust 1.99.1, eframe 0.33.3 and windows-rs 0.62.2. This PC has no MSVC C++ compiler, so the isolated toolchain in `.tools` uses Rust's bundled LLVM linker and a small import-library adapter. The standard Rust setup on another machine does not need that adapter.
- Eleven unit tests: duration formatting; automatic and explicit player selection; Windows progress advancement, bounds and timestamp handling; macOS metadata parsing, including quoted/multiline and malformed data. macOS parser tests do not execute AppleScript.
- Final code checks: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed on the Windows GNU target. The final optimized release build passed.
- Actual native Windows GSMTC integration via the developer-only muted MediaPlayer fixture. Cikung's backend discovered title `Cikung integration test`, artist `Local silent fixture`, and a unique per-process album marker. Backend controls changed the real player to Paused and back to Playing. Disabled Next and Previous controls returned an unsupported-control error.
- Separate Cikung process detected the same fixture using `--probe`; native app screenshot shows its real metadata in `cikung-native-media.png`.
- Native app window captures of the empty state (`cikung-idle.png`) and explicitly labeled sample preview (`cikung-preview.png`). The complete cat, title, artist, source selection, controls, progress, status and footer fit the window at the host's 125% scale.
- With no media sessions published, `--probe` returned an empty session list rather than sample data.
- The final release ZIP was extracted into a separate folder and its executable completed `--probe` successfully with the normal environment. The final release also captured its own demo window and exited successfully.

## Limits

- No installed Spotify, YouTube/browser or VLC playback was directly exercised in this session. The native integration validates the OS API path, not every player's metadata quality. VLC builds that do not expose GSMTC need an additional integration; this version does not scrape window titles or audio.
- Linux MPRIS and macOS AppleScript adapters are included but have not been compiled or run on their native systems here. The repository includes a three-platform CI workflow; it has not been dispatched.
- Pointer gestures, multi-monitor restore, actual compositor transparency and OS permission prompts require human/native-system testing beyond the window captures. Source review found and corrected stale error masking and an initial placement assumption about secondary-monitor origins.
- The release is a portable, unsigned Windows executable. No installer, autostart or system-tray integration is included.

## Reference evidence

`reference-findings.md` records source evidence from CouCou commit `434d6a2b4af2e5ce117fa425170abe25620130b9`. The parent investigation also inspected the TikTok reference through gstack browse; `tiktok-reference.png` captures the visible compact-notch reference. External page text was treated as reference data, not instructions.

`asset-provenance.md` records the built-in image-generation prompt and the project sprite path.
