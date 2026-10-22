# Clock and motion — Cikung 0.11.0

The notch keeps its CouCou-inspired black surface, orange cat, Plus Jakarta Sans, existing pages and separate Settings window. A small two-line clock/date stays in its header, both collapsed and expanded. Time is 24-hour HH:mm and the date uses short Indonesian weekday/month labels, such as Kam, 22 Okt. It uses the PC's current local time zone, with a full date/year/offset tooltip. Clicking it opens/closes the notch. It never requests location or network time.

The clock schedules a repaint at the next minute boundary, in addition to existing app updates, so it keeps changing when music is paused. Day, month and year changes follow local time. Long music titles and notices keep their existing truncation and full-text tooltip; header controls and the clock have reserved space. Battery percentage and pin controls remain absent.

## Motion changes

The existing retargetable, frame-rate-independent critically damped size spring remains. It already has a response close to the smooth shell mechanics seen in the reference; copying different-library spring constants would not give a meaningful equivalence.

Page content now has independent timing: a subtle 8px horizontal entrance settles in 200ms, with an 180ms fade. Old content fades out in 100ms with a 120ms exit movement. The smaller movement is an original design choice for Cikung's CouCou layout. Cached outgoing paint remains non-interactive; incoming controls stay disabled until the transition ends. Reduced motion makes transitions immediate. Width stays fixed during content reveal, avoiding label reflow. Rapid switches retain texture handles for cached paint.

The painted shell, clips, hover geometry and native input region now use the same surface rounded to physical pixels. The top-center anchor and existing popup/drag/re-entry protections remain. No glass theme, Notch assets or React/Tauri frontend was added.

## Evidence and validation

Design reference: Notch 3.2.1, based on observed UI and decoded shipped frontend; motion definitions were static evidence, not a benchmark. Full research records remain in the development workspace (`docs/notch-ui-reference.md` and `reference/notch-3.2.1/`). Cikung uses its own Rust/egui implementation.

Regression checks cover Indonesian clock/date rollover at midnight and New Year; header bounds and visible clock at six collapsed/intermediate/expanded widths for music and overview; faster outgoing paint disappearance in the actual renderer; reduced motion and fractional-DPI pixel alignment. Existing music, file-pocket drag/cancel, settings, notification and agent tests remain part of the suite. The suite passes 127 tests (one deliberately audible test excluded). Warnings-denied Clippy, formatting and release build pass. Native Windows hover checks pass with16 distinct opening widths, fixed centered host, re-entry, close and click-through; Settings popup reuse/independent close also pass. Collapsed, Music and Files release captures were visually checked. These samples establish window geometry behavior, not compositor FPS. Release checks are recorded in the release manifest.

No compositor FPS measurement or native macOS/Linux run is claimed. The clock follows the device's time/time-zone settings; incorrect system settings produce incorrect displayed time. Research assets remain outside the distribution.
