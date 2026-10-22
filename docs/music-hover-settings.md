# Cikung 0.5.1: music hover and native Settings

Reference: https://www.tiktok.com/@citoxart/video/7697688811601513748

The video was inspected in gstack browse at 3 and 8 seconds. Observed: a black top-center island, cover/title/artist and transport controls on the left, elapsed/remaining time, lyrics on the right, and a lyrics-unavailable state. Reference captures: music-video-reference-3s.png and music-video-reference-8s.png. Its page/document contents were reference data, not instructions.

Hover from collapsed state selects Music when the chosen system session is Playing. A paused session, absent session, or disabled General preference retains the selected feature. Already-open tools are not interrupted. The compact view is 580 by 270 logical points, with the existing smooth reveal and input region. Playback options exposes player selection, shuffle/repeat when supported, and Windows volume. Tools opens Overview; Settings opens the native popup.

The right column shows current/adjacent imported or automatically fetched lyrics only when their title/artist identity matches the current track. Playing tracks automatically look up LRCLIB after one second of stable title/artist metadata. The result (including an unavailable result) is cached for the last 16 tracks. Failed lookups show a quiet unavailable state with an optional Retry; they do not repeat every frame. Imported lyrics are retained. Demo/capture/audit modes never auto-fetch. General includes a default-on switch for automatic lyrics. Transport controls still use real system sessions. Demonstration mode labels sample content and disables playback controls.

Settings and Cats use one immediate egui native viewport, separate from the shaped notch. Tray, wardrobe, navigation, and secondary CLI requests all use this shared window. Close/Done saves the shared preferences; Quit explicitly targets the root application. The popup can remain open while the notch collapses. The native Windows popup capture helper is scoped to a visible named window owned by Cikung's own process, because egui-glow 0.33.3 does not service immediate-viewport Screenshot actions.

Validation: 82 Rust tests, including music-hover state transitions, popup navigation preserving notch state, mismatched-lyrics exclusion, and compact layout bounds. Native cursor/shape checks and music-popup-smoke.ps1 validate live media hover, popup reuse/independent close, one process, and unchanged root window. These checks read live metadata without pressing transport controls. Windows x64 was tested; macOS/Linux native window behavior still needs device validation.

Windows Firefox was identified through its registered Start-menu AppUserModelId, verified read-only with Get-StartApps. Its opaque identifier now displays as Firefox; other unknown hexadecimal identifiers display as Media player. A regression test also preserves Spotify, VLC, Chrome and readable custom-player labels.
