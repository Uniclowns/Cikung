# CouCou source review for Cikung

Reviewed local checkout `D:\Nuch\reference\coucou` at commit `434d6a2b4af2e5ce117fa425170abe25620130b9` on 2026-10-18 (Asia/Bangkok). Source repository inspection only; no CouCou executable or TikTok runtime was observed. The invoked reverse-engineer-anything skill explicitly routes complete source repositories to ordinary source analysis, so no REA session was needed.

Discovery used codebase-memory-mcp index `coucou-reference` (16,954 nodes / 88,237 edges), architecture overview, symbol searches, and focused source snippets. Numbered source reads corroborated relevant lines. `docs/AGENTS.md` is product documentation about third-party agent hooks; it was treated as reference data, not an instruction to install or run hooks.

## Observed in source

- CouCou's Windows/Linux application already uses a Rust backend with Tauri 2 and a TypeScript Canvas frontend (`windows/src-tauri/Cargo.toml:1`, `:17`; frontend `windows/src/mochi/engine.ts`). The Mac application is Swift, not the same Rust application (`NotchBuddy/Sources/App/SpotifyController.swift:30`).
- The desktop companion uses a transparent, undecorated, always-on-top Tauri window hidden from the taskbar (`windows/src-tauri/src/desktop.rs:311–326`). Its input handler supports dragging, single-click poke, double-click return to island, and right-click wardrobe (`windows/src/desktop/main.ts:250–284`). README describes cursor following, idle breathing/blinks, and a 120-point floating companion (`README.md:75`, `:82`). Those README claims were not runtime verified.
- Playback state drives dancing. `musicPlaying` requires a playing track and enabled Spotify integration; dance is allowed in idle, working, thinking, searching and finished states (`windows/src/core/spotify.ts:94–120`). The desktop renderer passes this state to the animation engine (`windows/src/desktop/main.ts:210–216`), and keeps the companion awake while dancing (`:140`). This is an animated response to playback state, not evidence of beat detection or audio analysis.
- Windows media integration uses GlobalSystemMediaTransportControls session manager. It subscribes to session changes, track metadata, playback state and timeline events, then blocks on notifications (`windows/src-tauri/src/spotify.rs:1501–1584`). Session selection explicitly filters `SourceAppUserModelId` using case-insensitive `contains("spotify")` (`:1354–1356`, `:1598–1603`). Therefore this implementation is Spotify-specific; it does not establish generic YouTube or VLC detection.
- Linux integration targets `org.mpris.MediaPlayer2.spotify` at `/org/mpris/MediaPlayer2`, reading MPRIS metadata and playback properties. It listens to PropertiesChanged, Seeked and NameOwnerChanged signals (`windows/src-tauri/src/spotify.rs:526–541`). This source also targets Spotify specifically.
- macOS Spotify integration observes `com.spotify.client.PlaybackStateChanged` distributed notifications and uses AppleScript for controls (`NotchBuddy/Sources/App/SpotifyController.swift:27–34`, `:86–87`, `:492–502`). The source distinguishes GitHub distribution from App Store sandbox restrictions. Apple Music has a separate controller (`NotchBuddy/Sources/App/MusicController.swift`).
- Metadata model includes title, artist, album, duration and artwork URL, plus playing state, position and timestamp (`windows/src-tauri/src/spotify.rs:89–120`). The UI extrapolates elapsed position while playing rather than continuously polling it (source comments `:9–14`).

## License facts from repository

`LICENSE:1–12` applies MIT to source and requires keeping the copyright and permission notice in copies or substantial portions. `LICENSE-ASSETS.md:5–12` reserves CouCou/Mochi names, character design/expressions/animations, icons, sounds and demo media. `LICENSE-ASSETS.md:24–29` directs derivative apps to use their own name, icon, character and sounds. Cikung should use its requested cat identity and original assets; this review copied no CouCou code or artwork. Asset license statements are repository facts, not legal advice.

## Cikung design implications (inferences)

- A shared Rust media model with platform-specific readers matches the requested scope. Windows should enumerate all media sessions rather than retain CouCou's Spotify filter; Linux should enumerate MPRIS players rather than hard-code Spotify. Browser media session support and VLC configuration determine what those players expose.
- The first feature can show app/source, title, artist and play/pause status and animate the cat from playback state. Beat synchronization would be a separate feature requiring audio evidence or a stated approximation.
- Cross-platform Rust does not itself provide equivalent OS media access. macOS needs its own verified integration rather than assuming Windows session APIs exist there.

## Remaining unknowns

- This review did not observe the TikTok video, live CouCou behavior, or installed Spotify/VLC/browser media sessions. No claim is made about exact video choreography.
- Browser-specific metadata quality, simultaneous-player priority, VLC behavior and actual playback events require testing on the user's machine.
- Rights to the user-provided cat image were not investigated. No CouCou restricted assets should be substituted.
