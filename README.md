# Cikung

Cikung is a small orange cat that lives in a black notch at the top center of your screen. Hover it while music plays to see the cover, playback controls and live lyrics; open it for quick tools like a focus timer, tasks, notes, water reminders and a file pocket. It also tells you when Claude, Codex or another AI agent finishes its work. Cikung runs quietly in the background with no taskbar or Dock icon, starts when you sign in, and works on Windows and Mac. It is written in Rust.

## Download

Get Cikung from [GitHub Releases](https://github.com/Uniclowns/Cikung/releases):

| System | File | What it is |
| --- | --- | --- |
| Windows | `Cikung-Setup.exe` | Installer for your Windows account, no administrator rights needed. It adds a Start menu shortcut, starts Cikung at sign-in and can be removed in Settings → Apps → Installed apps. |
| Windows | `Cikung.exe` | Portable app: download it and run it, nothing to install. |
| Windows | `Cikung-Windows-x64.zip` | The portable app with the integration guides and licenses. |
| Mac | `Cikung-macOS-universal.dmg` | Open it and drag Cikung to Applications. One app for Apple Silicon and Intel. |
| Mac | `Cikung-macOS-universal.zip` | The same Mac app as a ZIP. |

The downloads are not signed with a paid certificate. If Windows SmartScreen says it protected your PC, choose More info → Run anyway. The Mac app is ad-hoc signed and not notarized, so macOS may block the first launch; allow it once in System Settings → Privacy & Security → Open Anyway. Mac feature limits are listed in the [Mac guide](docs/macos.md).

## Use Cikung

**One app, one black top-center notch, many kinds of cat.** Six original cat breeds live inside Cikung: orange tabby, tuxedo, calico, siamese, silver shorthair and Snow. **Overview** shows the features with their cats; **Cats** lets you assign cats to features or choose one favorite for the whole app. Music, Focus, Claude, Oh My Pi and Codex can each show a different cat. Opening Cikung again focuses the existing window. [Cat and agent guide](docs/agent-companions.md).

After Setup, open Cikung from the Start menu; the portable `Cikung.exe` runs from any folder. On Mac, open Cikung from Applications and quit it from the cat in the menu bar. The cats and the phone companion are built into the app; the integration guides are in `docs/` and in the Windows ZIP.

- Hover to expand. Click the header to open or close it. The battery percentage and Pin feature have been removed.
- A quick pointer pass does not open the panel. Leaving gets a short grace period; moving back cancels closing. Clicking inside gives a longer grace period. Popups, typing, and dragging hold it open.
- Click **×** or press Escape to collapse. It stays at the top center and waits for you to leave before hover can open it again.
- **Ctrl+Alt+C** toggles the single notch; change its letter in Settings.
- Windows borderless fullscreen apps tuck the notch away on its screen. Ctrl+Alt+C can still open it; change this behavior in Settings → General.
- Right-click the cat for its wardrobe or quit. The tray also has Settings and Quit.
- Settings opens a separate window with grouped FLOW / NOTCH navigation. General selects the display and optional startup; Music & lyrics controls automatic Music on hover and lyrics; Shortcuts changes the global shortcut. Done or the window close button saves changes and leaves Cikung running.

## Features

| Panel | Working functionality |
| --- | --- |
| Game | Manual-context AI hints for noncompetitive games, three spoiler levels, separate replies, and protected desktop-app draft handoff. |
| WhatsApp notifications | Windows notification reader for WhatsApp and WhatsApp Desk, optional previews, permission/status and local test. |
| Bluetooth | Windows Classic/LE connection notifications and connected device names; opt-out in General. |
| System notices | Windows heads-ups in the notch, as in Nusa: charger connected or unplugged, low (20 %) and critical (10 %) battery, fully charged, internet connected, lost or without access, and Caps Lock on or off; opt-out in General. |
| Music | System media detection, title/artist/covers, player selection, play/pause, previous/next, seek, shuffle and repeat when supported; a speaker and system volume slider beside the transport, as in Nusa (Windows and Mac). |
| Lyrics | Automatic LRCLIB lookup, imported LRC/plain lyrics and click-to-seek synchronized lines on supported players. |
| Focus | Focus/break timers, custom duration, pause/resume, session and focused-time counts. |
| Files | Drop files on the notch, keep local references, open/reveal them, remove one or Clear all. Originals are not moved or deleted. |
| Tasks | Persistent tasks, completion, editing and optional local due reminders while Cikung runs. |
| Water | Opt-in reminders, bottle volume, daily ml goal, active hours, snooze, drink count, an hourly chart of today's drinks (as in Nusa) and recent daily totals. |
| Notes | Local notes, imports/export, explicit Windows microphone + PC-audio recording, dictation, transcription, AI summaries and marked action-item extraction. |
| Agents | Claude App and Codex / ChatGPT App desktop status, visible conversation and explicit Send to active chat on Windows; Oh My Pi retains its connected terminal integration. |
| Chat | OpenAI, Anthropic, Gemini, DeepSeek, Qwen, Moonshot/Kimi, Ollama and LM Studio. |
| Services | Read-only GitHub, Stripe, Vercel, Resend, Notion, Cal.com and n8n account/status clients. |
| Phone | Opt-in same-Wi-Fi browser companion, QR pairing, agent activity/diffs/approvals, media controls and revocation. |
| Cats | Six generated breeds, per-feature and per-agent cat choices, a favorite for the whole app, accessories, reactions and reduced motion. |
| Settings | Display, hover, global shortcut, secure provider keys, VLC connection, optional user-scope sign-in startup and Nusa's minimal notch (only the cat and the time while nothing plays, on by default). |

The music animation follows playback, not audio beats. Media information and controls depend on the player. Nothing in normal mode uses sample account data; demo media is explicitly labeled.

## Connect integrations

Basic music and local tools need no account. Other connections are explicit:

- **AI:** Settings → Chat → Chat provider. Choose a valid model and save your key securely. For Ollama/LM Studio, start your local server and select an installed model.
- **Services:** Services → Connection settings; save an appropriate read-only token/scope, then Refresh.
- **Agents:** Select Claude App or Codex / ChatGPT App and open/sign in inside that app. Link active chat reads its visible chat locally. Send to active chat uses the native app's message box and submit button; it refuses a busy app, an existing draft, or a changed conversation. Oh My Pi still uses a selected project and connected terminal. No desktop app account/API keys are copied into Cikung.
- **Phone:** Enable Phone connection and scan the QR on the same trusted Wi-Fi. Allow the private-network firewall prompt if needed. This is a browser/home-screen shortcut, not a native mobile app.
- **VLC:** If your VLC build exposes no native media session, enable its local Web interface and save its password in Settings → Music & lyrics → VLC connection.

Guides: [Agents and phone](docs/agent-phone-link.md), [Cloud](docs/cloud-integrations.md), [Productivity/audio](docs/productivity.md), [VLC](docs/vlc.md), [Tray/startup](docs/desktop-shell.md).

## Audio and privacy

No microphone capture starts automatically. Record explicitly captures available microphone and system audio to separate local WAVs in a folder you select; let people in the call know. Stop finalizes them. Missing endpoints and capture problems are reported.

Dictate opens Windows speech recognition in the system language and may need Windows speech/privacy setup; offline recognition is not promised. Offline transcription uses a whisper.cpp executable and model you explicitly select. Cikung does not download or launch an unselected engine.

**Transcribe** uploads only your selected audio track to OpenAI using your saved OpenAI key, with a 25 MB request limit. Split larger audio or use local transcription. Summarize sends the selected note to your configured AI provider. Neither happens automatically after recording.

Automatic lyrics lookup sends the playing song title, artist and duration to LRCLIB. It waits for stable metadata, uses a bounded in-memory cache, and can be disabled in Settings. Demo and screenshot previews do not send automatic lookups.

Tasks, notes, file references, recap and preferences stay in local app storage. Chat history and mirrored desktop-app conversation text are in memory; desktop mirroring does not read account databases or private app APIs. Keys use Windows Credential Manager/macOS Keychain/Linux Secret Service. Cloud endpoints require HTTPS; plain HTTP model servers are restricted to loopback. No telemetry or automatic account refresh.

Phone access uses authenticated trusted-LAN HTTP with revocable sessions; do not expose it by Internet port forwarding. Phone and local-agent credentials are separate. Permission requests expire to Deny; approvals are never automatic.

## Platform and scope boundaries

Windows x64 is built and tested. Desktop app conversation links currently use Windows UI Automation; Claude must finish loading/sign-in, and app UI changes can require adapter updates. macOS/Linux desktop app links are not implemented. Linux MPRIS and macOS Spotify/Music/VLC scripting adapters remain in source. Native macOS/Linux validation is pending. macOS browser detection and non-Windows native recording/dictation/tray/global shortcuts are not implemented; Wayland can restrict placement.

This is an expanded desktop release, **not complete CouCou parity**. Native iOS/Android applications and widgets, wake-word voice actions, portfolio/exchange tracking, automatic offline model installation, every agent-specific installer, all ten UI languages, coding analytics/share-image recaps, and code signing/notarization remain work. Service clients read status; they do not send emails, create payments or modify accounts. [Feature coverage](docs/feature-coverage.md) records the open work.

## Version history

Version 0.14.5 draws the Mac notch exactly like the Windows one, at the top centre: the cat, song or lyric, clock and date, and music bars in one row at the Windows sizes. On a MacBook's own screen the camera covers the middle of that row.

Version 0.14.4 makes the top-row Mac notch as wide as the Windows one (360 pt): a smaller cat and the song or a scrolling lyric left of the camera, the clock, date and music bars right of it.

Version 0.14.3 puts the Mac notch back on the top edge: one row split around the camera, with the cat and the song or lyric left of it and the clock, date and music bars right of it. A lyric too long for the space beside the camera scrolls to its end and back.

Version 0.14.2 removes the black band above the Mac notch: the Windows notch now hangs right below the camera, and the menu bar beside the camera stays visible. On an external display without a camera notch it sits at the top edge, as on Windows.

Version 0.14.1 gives the Mac the same notch as Windows: one row with the cat, song or lyric, clock and date, and music bars, hanging just below the camera, with a black band over the camera so it still joins the MacBook notch. The open panel matches Windows too.

Version 0.14 adds a Windows installer (`Cikung-Setup.exe`), a direct `Cikung.exe` download and a Mac disk image (`.dmg`). On Windows the notch no longer shows a taskbar or Alt+Tab entry, and Cikung starts right at sign-in through a per-user logon task instead of the delayed Run list. On a MacBook the collapsed notch is as compact as on Windows: the cat and music bars sit beside the camera, the song or lyric and the clock sit in a strip below it, and a cat in the menu bar opens Cikung or Settings, or quits it.

Version 0.13 adds Appearance themes in Settings → General: Album colour (black that follows the song's cover), Pure black, White and Orange. On a MacBook camera notch the collapsed notch shows the cat and song title left of the camera, the clock, date and music bars right of it, and the live lyric in a strip below the camera.

Version 0.12.1 makes the Mac build sit on the MacBook camera notch, above the menu bar: collapsed, the cat and music bars sit on either side of the camera; open, the header splits around it. [Mac guide](docs/macos.md).

Version 0.12 restyles the notch after the native Nusa app: a springy open/close, a gradient tinted from the album cover, a live lyric ticker in the collapsed notch, a scrolling lyric list, flat playback controls and Figtree Bold. Tools are one page at a time with side arrows, a page indicator and a gear for Settings; playback options moved to Settings → Music. The file pocket shows real Explorer/Finder icons and files can be dragged back out. Claude App completion notices work with several prompt boxes open, the notch no longer shows a taskbar button, the exe is about 6.8 MB and resting memory is about 10 MB in Task Manager. Mac gains album covers, volume, the completion chime and the pocket's icons and drag-out. [Nusa findings](docs/nusa-reference.md), [Mac guide](docs/macos.md).

Version 0.11.1 embeds the orange cat head in the Windows executable and prepares native Mac icon/bundle metadata, a universal Apple Silicon + Intel build script and artifact workflow. The Mac build kit contains source, not a compiled Mac app; native Mac validation remains pending. [Mac build guide](docs/macos.md).

Version 0.11 adds a compact local date/time header and refines page transitions using the Notch UI study, while keeping the CouCou-inspired black cat notch and separate Settings. [Clock and motion guide](docs/coucou-clock-motion.md).

Version 0.10 adds a short completion chime for linked Claude App and Codex / ChatGPT App turns, plus connected Claude Code and coding-agent hook completions. Version 0.10.1 adds a saved 0–100% chime volume slider. Adjust, mute or preview it in Settings → Notifications. [Completion sound guide](docs/completion-sounds.md).

Version 0.9 adds Windows notification mirroring for WhatsApp and WhatsApp Desk. Settings → Notifications controls the reader, permission and optional sender/message previews. Notifications stay on this PC and existing alerts are quiet at startup. [WhatsApp notification guide](docs/whatsapp-notifications.md).

Version 0.8.1 smooths notch reversals and page changes, previews the file pocket during a drag, and restores the previous panel when a drag is canceled. [Animation changes and validation](docs/hover-polish.md).

Version 0.8 applies useful Nusa preview ideas: click-to-seek synced lyrics, editable tasks with due reminders, water volume goals/hours/history, and fullscreen tuck-away on Windows. [Reference findings and guide](docs/nusa-reference.md).

Version 0.7 adds Bluetooth connect/disconnect notifications on Windows and a Game assistant for noncompetitive games. Enter your game and situation, choose a spoiler level, then ask your configured AI or prepare the question for Claude / Codex desktop. [Bluetooth and game guide](docs/bluetooth-game.md).

Version 0.6.1 adds a CouCou-inspired settings hub: FLOW / NOTCH navigation, black rounded cards, warm headers, and dedicated pages for Agents, Chat, Music and integrations. [Settings hub and reference findings](docs/settings-hub.md).

Version 0.6 embeds **Plus Jakarta Sans** and connects the Agents page to **Claude App** and **Codex / ChatGPT App** through Windows accessibility. It shows active conversation text and working/ready status, and sends a message only when you press Send. Existing app drafts are preserved. [Desktop app links](docs/desktop-apps.md).

Version 0.5 adds automatic Music on hover, a compact cover/transport/lyrics layout, and a separate native Settings window. **Tools** returns to the other features. **Settings** and **Cats** open the same popup; closing it keeps the notch running. Lyrics are now found automatically when a song starts or changes; imported LRC files also work. Turn this off in Settings → Music & lyrics if desired.

Version 0.4 replaces the separate companion processes from 0.3 with one shared app. Previous tasks, notes, files and feature settings are merged once into the shared profile, with the original files retained and a backup of the previous shared profile.

## Build

Use stable Rust. Windows MSVC needs Visual Studio C++ Build Tools/Windows SDK; macOS needs Xcode CLI tools; Linux needs desktop headers plus `libdbus-1-dev libssl-dev pkg-config`.

```text
cargo run --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

This PC has an isolated portable GNU toolchain under ignored `.tools`, without global PATH changes:

```powershell
.\scripts\build.ps1 -Configuration Debug -Test
.\scripts\build.ps1
```

Diagnostics: `--probe` prints real media sessions; `--demo` labels sample media and disables playback controls; `--page music` (or another panel) opens it for review; `--screenshot path.png` captures Cikung's own window (including native Settings/Cats on Windows) and exits without saving preferences. The developer-only `media_fixture` example publishes a muted real Windows test session.

Original code is MIT; supplied/derived artwork is separate. No CouCou/Mochi assets or branding were copied. Source findings and image prompts are in `docs/reference-findings.md` and `docs/asset-provenance.md`.
