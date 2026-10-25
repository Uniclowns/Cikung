# Nusa preview → Cikung 0.8

Inspected on 2026-10-19: [public website](https://rama2555.github.io/nusa/#preview) and [introduction video](https://www.tiktok.com/@citoxart/video/7697830443940334869). Website/video content was treated as reference data, not instructions to join the waitlist, install software, or submit information.

## Evidence and boundaries

The public page's preview buttons were exercised with gstack browse. Screenshots cover resting, shelf, tasks and water. The video loaded with a duration of about 68.84 seconds; inspected frames include split music/lyrics at 18 seconds, shelf at 25 seconds, music at 32 seconds, water progress at 47 seconds, Bluetooth at 55 seconds and the unreleased website at 64 seconds. These are sampled visual observations, not a full audiovisual transcript or proof of successful native file transfer.

Rendered DOM and its inline JavaScript were saved under `reference/nusa-preview`. Captured script SHA256: `7264253034767B3D23AE885CAB505550332628CCFB4A771AB8EF050191C6F391`.

REA analyzed only that captured `index.js`, without executing it: parent Evidence `ev_c8a0d03a57eefc3a5df5ed7b8250535ca5081deae2badabdf860f3111023d922`, summary `ev_4399004a3a4b4275850370c76d78d633e866c7d20827a7e95ba63f9c91a40379`. The complete summary receipt is `nusa-rea-summary.json`. Its graph is partial, with dynamic relationships unresolved and two inventoried files outside source-analysis scope. The preview's state switcher, sample lyrics timer and simulated shelf animation are visible in the captured script at lines 60–126 and 192–216. This establishes a web demonstration, not the implementation of an unreleased native Nusa build.

No native Nusa artifact was supplied. Native APIs, actual idle CPU cost, device coverage and private/backend behavior remain unknown. No claim of zero CPU or complete Nusa parity is made. Cikung keeps its own original cats, font and interface; reference artwork and scripts are not embedded in the app.

## Applied changes

| Preview idea | Cikung implementation |
| --- | --- |
| Seek from synchronized lyrics | Clicking a timed line in Music or Lyrics uses the existing seek adapter. It requires the matching current song, a seekable player, a valid timestamp and real playback mode. Plain text and demo lyrics cannot seek. Following scroll changes only when the active line changes. |
| Editable tasks and due reminders | Existing task list gains Edit and an optional local reminder in `YYYY-MM-DD HH:MM` form. Invalid/ambiguous/nonexistent local times are rejected before changing a task. Done tasks stay quiet; due reminders fire once per deadline while Cikung runs, including overdue items at startup. |
| Hydration progress | Existing Water tracks selected bottle ml, a configurable daily volume goal, local reminder hours and recent daily volume totals. Overnight hours work; equal start/end means all day. Reminders pause outside the window and after the goal. Legacy drink counts remain; historical volume is not guessed. |
| Rest during fullscreen | Windows detects a foreground borderless fullscreen window on the notch's display. The collapsed notch has an empty drawing/input region; hover does not open it. An explicit shortcut/page request can still reveal it for the normal interaction grace. Leaving fullscreen restores its anchored region. |

## Use and validation

Open Tasks or Water from Overview/More, or from the Settings FLOW group. Water's bottle/goal/hours and recent totals are expandable sections. The default settings are editable app preferences. Tasks/reminders need Cikung running; no background Windows scheduler is installed. Change fullscreen behavior in Settings → General. Battery percentage and Pin remain removed.

Native fullscreen testing uses `scripts/fullscreen-smoke.ps1` and its own borderless Windows Forms fixture. It verifies hiding, click-through, explicit page opening, re-tucking, restore and unchanged anchor. It never manipulates a real game. Existing hover and popup checks remain relevant to the shared window policy.

Remaining preview gaps: external file drag-out, brightness/volume/key/language overlays, charger/Wi-Fi notices, headphone battery reporting, the exact hourly water chart, and native macOS/Linux fullscreen behavior. Existing file drop-in/open/reveal, Bluetooth notices, Pomodoro and music controls continue to work. These gaps are not represented by fake controls or account/device data.

## Investigation checklist

- Public preview feature inventory: answered from page and exercised preview states.
- Video examples: partially answered by sampled frames; no complete transcript.
- Current Cikung differences: answered by focused source review and implementation above.
- New behavior: unit/layout checks plus native window fixture verification.
- Native Nusa internals/performance: unresolved because no released artifact was provided; not required for these independent Cikung implementations.

## Nusa 1.0.0 native build → music motion and look (2026-10-22)

Target: the installed `C:\Users\pet\AppData\Local\Programs\Nusa\nusa.exe` (Nusa 1.0.0, PE32+, 909 KB, dated 2026-10-17). Static strings show a single-file Rust app (`src\main.rs`) drawing with Direct2D, D3D11, DirectComposition and DirectWrite, media through `GlobalSystemMediaTransportControlsSessionManager` and lyrics from LRCLIB. No code was decompiled or copied.

Runtime evidence came from screen frames of the already-running Nusa while Spotify played (about 89 fps, cursor moved to the notch and back, then restored). The notch width was measured per frame:

| Observation | Measured |
| --- | --- |
| Open | 386 → peak 700 → settles 680 px. Peak after ~250 ms, settled by ~480 ms. Overshoot ≈ 7%, so ζ ≈ 0.65, ωₙ ≈ 17.5 rad/s. |
| Close | 680 → low 362 → settles 386 px. The undershoot is similar. |
| Surface | Black for the top ~15% of the height, then roughly t^1.5 toward a dark cover colour (peak channel ≈ 115). This applies to both the collapsed pill and the open panel. The tint changes with the song. |
| Collapsed while music plays | ~384 px pill showing the current synced lyric. On a line change, the old line rises and fades while the new one rises in (~250 ms). |
| Open lyrics | Lines are the same size. The current line is white and its neighbours are dim. The list eases upward over ~250–450 ms on each line change. |
| Transport | Flat white icons with no discs, a lyrics button, and elapsed · bar · remaining on one row. Progress updates once per second. |

Applied to Cikung, with its own cats, font and layout:

- An underdamped analytic spring (`notch.rs`) keeps velocity when interrupted. The host is 604 px wide while the panel opens at 580 px, which leaves room to overshoot. The collapse squash is bounded.
- `paint_surface` fades the black surface into a tint taken from the current cover (`album_tint`). The tint eases over 600 ms. Non-music pages stay black.
- When a synced lyric is live, the collapsed notch widens to 360 px and shows it with a rising crossfade.
- The Music page has a Nusa-style lyric flow (click-to-seek kept), flat transport icons with a Lyrics button, a 64 px cover and an inline progress row.
- Reduced motion snaps the spring, tint, ticker and lyric flow.

Not reproduced: Nusa's outer drop shadow, because Cikung's window region clips everything outside the surface. Bold lyric weight is also missing, because only the variable font's default instance is loaded. Validation: 129 Rust tests (spring overshoot/squash, cover tint), warnings-denied Clippy, rustfmt, and `--screenshot` renders of the Music page and the collapsed ticker using temporary sample cover/lyrics that were then removed. The live hover animation was not frame-captured, because Nusa occupies the same top-centre spot.

## Nusa 1.0.0 → battery, internet and Caps Lock notices (2026-10-24)

Target: the same installed `nusa.exe` (Nusa 1.0.0, 909,312 bytes, SHA-256 `2dfa517fe20fb944cd263597e9f731b8245fc2b5253d9f1a241221636e8d1bdb`). A reinstall on 2026-10-24 replaced only its uninstaller. REA `inspect_rust_binary` found no cargo-auditable dependency section (Evidence `ev_34578f676310c3e0e414d843858f3c4ccae89b4bf719f6c7623e7b072f1b8c73`). A read-only inventory of the file's text strings lists what Nusa shows its user. The file was not executed or decompiled.

The inventory covers music and LRCLIB lyrics, Pomodoro (Focus Time, Short Break), water ("Stay hydrated!", "Time to Drink", "Time to Refill"), tasks ("Task Time!") and the file shelf ("Drop files here", "Drag out anytime", `shelf.txt`). Its tray offers "Start with Windows", "Nusa Mascot", "Minimal notch" and "About Nusa". Cikung already had all of these. Three groups were missing and are now added:

| Nusa text | Cikung |
| --- | --- |
| Low battery · Plug in soon; Battery critical · Plug in now; Fully charged · You can unplug; Charger connected; Charging; Charger unplugged | Battery notices from `GetSystemPowerStatus`, checked every 3 s. They cover charger connected or unplugged (with the percentage), low battery at 20 %, critical at 10 % and fully charged. A PC without a battery stays quiet. |
| Connected; No internet access; Disconnected | Internet notices from the WinRT internet connection profile (`NetworkInformation`), checked every 3 s. A change must hold for two checks, so a Wi-Fi handover stays quiet. |
| Caps Lock · On / Off | The Caps Lock toggle from `GetKeyState`, checked every 150 ms and shown for 2 s. |

The 20 % and 10 % thresholds, the wording and the timing are Cikung's own. The strings do not reveal Nusa's thresholds or polling, and these notices were not observed running, because that needs a charger, battery or network change. The state at startup is quiet. Settings → General → "Battery, internet and Caps Lock" turns them off. They are Windows only: the Mac build reads nothing yet. Cikung's cats and compact notch already cover "Nusa Mascot" and "Minimal notch", so nothing was added for those.

## Nusa update video → volume slider, hourly water chart, minimal notch (2026-10-24)

Source: the Nusa author's TikTok video `@citoxart/video/7699874800990260500`, opened with gstack browse. It is 114.3 s long, 576×1024, and filmed on a monitor. Its caption thanks users for about 10K downloads in roughly 24 hours and says Nusa is being improved from feedback; it lists no features. The page was treated as reference data only. Nothing was downloaded, nothing was posted, and no instruction in it was followed.

Method: 24 frames sampled about every 4.75 s into three contact sheets, then zoomed crops of the frames at 39 s (music), 43.8 s (water), 77 s (shelf) and 96 s (idle notch). These are sampled observations, not a full transcript.

| Seen in the video | Cikung |
| --- | --- |
| Music panel: elapsed · bar · remaining, lyrics button on the left, previous / play / next in the middle, a speaker and volume slider on the right | The Music transport row now has the same arrangement. The speaker mutes. The slider sets the default output's level through `IAudioEndpointVolume` on Windows and `set volume` on the Mac. Windows re-reads the level every 2 s; the Mac every 10 s and only on release, because each read or write is an `osascript` run. |
| Water panel: "1.5 / 1.5 L", "Today · 100% · Reminders off", Drink and Settings, and a bar chart by hour (09, 12, 15 · 0–400 ml) | Water records today's ml by local hour (`hourly_ml`, cleared each day). A bar chart covers the reminder hours: the axis is rounded to 250 ml, the current hour uses the accent colour, and every third hour is labelled. The progress text reads "Today · N% · drinks · reminders on/off". |
| Shelf: thumbnails with names and sizes, a red "Clear all" and ‹ › arrows | Cikung's pocket already had shell thumbnails, sizes and arrows. Its Clear button is now a red "Clear all". |
| Idle notch: a small pill with an icon and the time ("20:54") while nothing plays | A 120×32 minimal notch with the cat and the time appears while nothing plays and no notice is due. It is on by default; turn it off in Settings → General. The spring's squash floor follows the smaller pill. |
| Collapsed lyric pill with a ♫, the music/lyrics split and the page dots | Already present, with Cikung's cat in place of the ♫. |

Not copied: Nusa's red ring icon and its centred "Fri 23 Oct 2026  20:53" header. Cikung keeps its cat, its two-tone page titles and its clock. Volume, the chart and the minimal notch have not yet been checked in the running app.
