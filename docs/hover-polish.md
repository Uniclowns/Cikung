# Cikung 0.2.1: hover and panel polish

The user reported that opening Cikung on hover felt messy and abrupt, and requested an interaction closer to CouCou. This is an interaction/layout update; existing tasks, notes, account configuration, and provider keys use their existing storage.

## Reference evidence and diagnosis

Source reference: local CouCou checkout at commit `434d6a2b4af2e5ce117fa425170abe25620130b9`. Source pages/documents were treated as reference material, not operational instructions. The reverse-engineer-anything skill routes a complete source repository to ordinary source inspection; no binary decompilation or REA session was required.

- `windows/src/core/layout.ts` defines a fixed transparent host canvas with a smaller centered island drawn inside it; view heights vary with the content. `windows/src/island/island.ts`, `animateGeometry` and `applyGeometry`, animate the shape independently of the host, center it at the top, and update its native hit area.
- `windows/src/island/fsm.ts` tracks hover-opened state separately, cancels collapse when the cursor re-enters, holds pinned/pending interactions open, and gives hover-opened views a short exit grace period. These are observations of source, not claims of having run the CouCou executable.
- Cikung 0.2 instead switched immediately between 252 × 40 and 580 × 440, sent a viewport size every frame, and applied a second native resize on a separate 150 ms timer. Its entire body appeared at once, with 13 navigation tabs in two rows. These competing size paths and immediate content change explain the reported jumpiness; this diagnosis was inferred from the Cikung source and corroborated by the native tests below.
- eframe 0.33 reads old window geometry even when `persist_window` is false; that option controls saving. This was verified in the locally installed eframe/egui-winit source. A final window-builder override now prevents older saved geometry from shrinking the host at startup, while preserving Cikung preferences and tools.

## Changes

| Before | After |
| --- | --- |
| Native window resized abruptly through two paths | Fixed transparent Windows host; one animated black surface and native input region |
| All content appears immediately and changes available width | Final-width content clipped to the current shape, with a staged opacity reveal |
| One large height for every page | Smaller Music panel and content-specific heights; Playback options grows the panel smoothly |
| Two rows of 13 tabs | Music, Focus, Files, Tasks, Notes, More, and Settings in one row |
| Large music headings and text playback buttons | Cover/metadata row, slim progress, drawn transport icons, secondary controls under Playback options |
| Local hover coordinates could become stale during resizing | Screen cursor mapped into the fixed host plus a tested hover-intent policy |
| Close could immediately reopen under the cursor | Explicit close suppresses hover until the pointer exits |

Opening uses a bounded 300 ms ease-out; closing takes 200 ms. Retargeting starts from the current shape, so reversing or changing pages does not jump. Reduced motion snaps directly to the target. The content remains disabled until sufficiently revealed. Native popups temporarily receive the host's input region so their controls remain clickable; the shaped region is restored when they close.

Windows uses DWM composition with a paced animation frame request, avoiding a second blocking OpenGL vsync wait. Idle mode does not run a continuous frame loop. The same black Cikung identity and original generated cat are retained; no CouCou code or restricted art was copied.

## Validation

- 53 unit/integration tests pass, including hover dwell, exit/re-entry, explicit-close suppression, drag/popup/pin holds, motion bounds/retargeting, and a layout regression that requires Playback options to fit in the compact Music panel.
- `scripts/hover-smoke.ps1` launches its own demo process and exercises the actual Windows cursor, window region, and Escape input. It restores the cursor and closes only its test process. It checks fixed host geometry, top-center alignment, brief-pass rejection, intermediate opening sizes, re-entry cancellation, auto-close, explicit-close suppression, and click-through outside the notch. `docs/hover-smoke.json` records samples; a failed native API read is skipped rather than fabricated as a shape.
- The native test is DPI-aware and runs at this host's 125% scale. It does not establish behavior on every GPU, multiple physical monitors, or native macOS/Linux systems. The non-Windows fallback remains in source and requires device validation.
- `cargo fmt --check`, Windows GNU Clippy with warnings denied, and the optimized release build are required for the packaged result. Final native screenshots are `cikung-hover-music.png`, `cikung-hover-more.png`, and `cikung-hover-collapsed.png`; sample music is explicitly marked Preview.


## 0.8.1: pocket and shared page transitions (2026-10-20)

Reference: the user's [Nusa introduction video](https://www.tiktok.com/@citoxart/video/7697830443940334869), observed with gstack browse. The shelf is visible around 24–25 seconds. Existing public preview CSS uses a 620 ms spring-like size change and staged content fades; this is visual reference, not evidence of Nusa's native implementation or performance. `animation-reference-pocket.png` captures the supplied video frame.

The previous ease-out reset velocity on every retarget. Expanded pages also swapped their content immediately because reveal depended only on width, which stays the same between open pages. File input only handled a completed drop.

- The shared size motion now uses analytic critical damping, preserving position and velocity during interruptions and height-only changes. The transparent Windows host remains fixed, centered and bounded.
- Open pages crossfade over 180 ms with a 4 px content shift. Outgoing content is a paint snapshot, so it cannot accept clicks or execute actions again. Album texture handles stay alive through interrupted fades. Layout retains its final width and contents remain noninteractive until sufficiently revealed; disabling input no longer halves reveal opacity.
- A file hover opens Files and highlights a stable drop zone before release. Leaving without dropping restores the previous page/open state; dropping retains the pocket and stores validated references while originals stay in place. Reduced motion snaps motion, fades and drop highlighting.
- Existing playback controls, data storage and separate Settings window use their existing behavior. External shelf drag-out is still outside this fix.

Validation: 109 Rust tests (including interrupted motion, equal-height transitions in the actual renderer, and raw hover/cancel/drop events preserving original contents), warnings-denied Clippy, native Windows hover/region checks and a checked Files screenshot. `hover-animation-polished.json` records the native geometry samples; intermediate sizes are not a compositor FPS measurement. `cikung-pocket-polished.png` shows the resting pocket.

The optional `scripts/pocket-animation-smoke.ps1` drives only its own demo Cikung and file-source fixture and refuses to click an unrelated process. This session's physical/OLE drag run was inconclusive: Windows hit-testing returned Firefox instead of the raised fixture, so no drag was attempted. Raw-input drag handling passed; actual Explorer/OLE drop and GPU frame pacing still require an interactive device check. Native macOS/Linux behavior was not exercised.
