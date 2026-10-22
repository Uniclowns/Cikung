# AI completion sounds — Cikung 0.10.1

A short two-note chime plays when a linked AI finishes a turn. It starts enabled on Windows. Open Settings → Notifications to turn **Play a sound when AI finishes** off or use **Test completion sound** to preview it. Set **Sound volume** from 0–100%; 0% is silent. The level is saved and applies to automatic alerts and the test button. Existing settings default to 100% to preserve the previous loudness. This changes only the embedded chime amplitude, leaving Windows and other apps unchanged. The test button is an explicit preview even when automatic sounds are muted, and respects the selected volume. A brief AI finished message also appears in the notch. Muting affects sound; it does not replay old completions when re-enabled.

## Sources

- **Claude App and Codex / ChatGPT App:** keep **Link active chat** enabled in Agents. Cikung must first observe Working, then two stable Ready snapshots for the same window, process and conversation title. The content fingerprint must stop changing. Idle apps, startup, loading, closing, pausing and attention states do not trigger a completion. The existing desktop link follows one active app/chat per target, not every background task or every open window.
- **Claude Code:** the existing connected Cikung hook adapter's Stop event publishes complete. Use the existing [Claude connection guide](claude-integration.md). An unconnected terminal cannot report completion to Cikung.
- **Connected Codex CLI / Oh My Pi sessions:** their existing complete events use the same sound. Permission requests, waiting, errors, Interrupt/SessionEnd/idle events alone do not create completion alerts. This update changes neither adapter permissions nor user app configuration.

New bridge completions are queued immediately at accepted events, rather than inferred from the latest displayed snapshot. Very fast complete → idle turns are retained. Duplicate complete events within a turn are suppressed, including repeats after an idle event; a subsequent working/thinking turn can complete again. The queue is bounded at 32. Desktop transitions are also retained while pending snapshots are drained. All events are consumed while muted; batches and alerts less than 750 ms apart share one chime.

The original 460 ms PCM sound is embedded in the executable. Windows plays it asynchronously through PlaySound; no external audio player, network request, model call or recording is needed. It uses the current Windows audio route and volume and does not pause user playback. Demo, capture and native-audit modes never play automatic completion sounds. A deliberate Test click can preview the sound.

## Practical limits

Desktop detection uses the existing visible accessibility link. A minimized/unexposed UI, a turn shorter than its polling interval, a title/chat change or a disconnected app may prevent detection. Completion normally arrives after the second stable Ready poll, not exactly at the last generated character. Desktop stop/cancel and a successful response can look alike; this is a turn-ended signal, not a guarantee of success. The explicit terminal hook complete event is more precise. A desktop view and a hook describing the same task can still create separate later alerts if they do not finish within the shared sound window.

Playback is implemented for Windows. The existing Rust application still builds its guarded non-Windows paths, but this feature does not claim native macOS/Linux audio support. System mute, the selected output device and Windows volume affect audibility.

## Validation

124 tests pass in the normal suite; one audible hardware test is deliberately excluded from routine tests. That native test was run separately and passed on this PC: Windows accepted playback of the embedded chime. The native isolated UIA fixture tests actual Stop generating / Ready controls and emits one completion; lifecycle, duplicate, startup, interruption, changed-window/chat, paused-link, burst suppression, waveform, volume scaling/clamping, zero-volume gate and saved-volume/mute regressions also pass. No test prompt was sent to a real AI app or CLI session. Real model turn completion remains unverified.

Primary audio reference: [Microsoft PlaySound function](https://learn.microsoft.com/en-us/previous-versions/dd743680(v=vs.85)). The scaled WAV buffer is retained for the lifetime required by asynchronous memory playback.
