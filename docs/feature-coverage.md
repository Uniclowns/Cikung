# Reference coverage

The requested direction is the full CouCou scope plus the Nusa/TikTok desktop tools. This is the current implementation boundary, including work that is still open; it is not a claim of complete upstream parity.

| Area | Implemented | Still needed |
| --- | --- | --- |
| Small cat, black top-center notch | Original head-only art, Windows display/DPI anchoring, device-pixel aligned surface, separate content enter/exit timing, smooth hover and collapse | Native macOS/Linux validation |
| Local date/time | Compact local 24-hour clock and Indonesian date in collapsed/expanded headers; minute refresh and year/zone tooltip | Follows PC clock/time-zone settings; native macOS/Linux validation |
| Cat types inside one app | Six original breeds; per-feature and per-agent choices, global favorite, one window/process, unified state and one-time profile migration | Native macOS/Linux validation |
| Settings hub | Reusable native popup, FLOW/NOTCH groups, fifteen real feature/settings pages, per-page scrolling and feedback | Native macOS/Linux validation |
| Desktop AI apps | Windows active-visible-chat/status links for Claude App and Codex/ChatGPT App, explicit Send with draft/context guards | App must expose accessibility; no full history sync, macOS/Linux pending |
| AI completion sounds | Original embedded Windows chime, saved 0–100% volume and mute/test control, stable desktop Working → Ready detection and authenticated hook complete-event queue | Desktop active-chat accessibility limits; real model turn completion and non-Windows playback remain unverified |
| WhatsApp notifications | Windows notification listener for WhatsApp and WhatsApp Desk, quiet startup, deduplication, optional previews and permission/status UI | Incoming real alerts from both apps still need live validation; requires Windows notification access and apps publishing Windows alerts |
| Bluetooth | Windows Classic/LE connection snapshots, debounced connect/disconnect notices, opt-out in General | Physical reconnection validation, other OS adapters; proprietary dongles differ |
| Game assistant | Manual game/situation context, spoiler preference, configured AI hints, guarded desktop-app draft handoff | User model/app setup; live model guidance, automatic screen context and voice |
| Music | Native metadata/control adapters, optional VLC HTTP, covers, seek/shuffle/repeat | User's real Spotify/browser/VLC version tests |
| Lyrics, focus, files, water, tasks | Local tools, editable due tasks, water volume goals/hours/history, LRC sync and click-to-seek, automatic lyrics lookup | Catalog matching depends on metadata |
| Character | Six breeds, per-feature assignments, four accessory styles, breathing/playback/poke reactions | Seasonal costumes, more expressions, character reaction sounds |
| AI chat | Eight real provider clients with secure keys | User account/local server setup and live tests |
| Agent monitoring/approvals | Authenticated protocol; Claude Code, Oh My Pi and Codex adapters; scoped connected terminal launcher; fail-closed decisions; Phone and Agents share one bridge | User CLI login, Codex hook trust, live coding-session validation; other agents |
| Service accounts | Seven real read-only account/status clients | User keys, live account tests; write actions not implemented |
| Dictation/notes | Native Windows speech, local mic/loopback WAV capture, explicit transcription, summaries and marked tasks | User speech/privacy setup, audio runtime test; local engine/model or cloud key; larger cloud recordings need splitting |
| Phone | LAN browser companion, QR pairing, approvals, media controls, revocation | Physical phone test; native apps, push, Live Activities/widgets, HTTPS for stronger installation |
| Desktop shell | Windows tray, notification balloons, optional sign-in startup, borderless-fullscreen tuck-away | macOS/Linux shell adapters |
| Nusa preview gaps | Reference website/video inspected; concrete feature mapping in nusa-reference.md | External shelf drag-out, system overlays, charger/Wi-Fi, exact hourly water chart, idle CPU claim validation |
| Recap | Local daily focus/task/water/note counts | Full coding analytics and share-image export |
| Wake word / voice service actions | Not implemented | Separate assistant lifecycle and explicit action confirmations |
| Portfolio/exchanges | Not implemented | Public-wallet and read-only exchange/chain adapters |
| Ten UI languages | Not implemented | Translation catalog and language QA |
| Distribution | Portable Windows package with embedded orange cat icon; Mac ICNS/universal bundle build script, source kit and artifact workflow prepared | Native Mac build/GUI verification, Developer ID signing/notarization, installers and mobile distribution |

Unconfigured services never show simulated account data. Development uses local fixtures and does not change autostart, record the user's microphone or mutate service accounts.
