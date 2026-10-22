# Running CouCou reference — 2026-10-19

Question: distinguish many character types inside one app from launching several copies of Cikung.

## Observed on this Windows host

- The user-selected running application resolved to `C:/Program Files/Coucou/coucou.exe`, version 0.5.0, process 7500 at observation time. SHA-256: `c6dfd152ec78be128f12f66015a73a6f4117f711d789990d43aa888f2c693106`.
- The process loads EmbeddedBrowserWebView.dll. Windows UI Automation exposed its `Coucou` web document, Overview/Ask/Drop/Settings controls, a `bot-canvas`, and wardrobe entries including None, Bow, Scarf and Witch hat. Accessibility includes hidden/overflow content, so enumerating a control does not establish that it was visible.
- One CouCou process owns the main Coucou window and additional internal windows including Mochi. This does not imply separate application instances per feature.
- An app-window capture is saved as `coucou-live-reference.png`. It shows the small main character and a compact group of colored mini characters in one host surface. The remaining black area belongs to the window capture; it is not a screenshot of the user's desktop.
- A UI Automation Invoke was performed on the observed Settings control. No settings, account connection, outfit, permission or agent action was changed.

## Source corroboration and inference

The existing source checkout's `windows/src/mochi/minibots.ts` creates small canvases with independent BotEngine instances, picks their body color from task metadata, and ticks them from the island frame loop. `wardrobe.ts` stores one wardrobe selection. These are source observations; the installed executable was not proven identical to that checkout.

Cikung design choice: keep one application and one top-center notch; provide original cat breeds, feature cards, per-feature/per-agent assignments and a global favorite. Do not create another process or notch when selecting a feature or breed. The generated cat art is Cikung's own character family; CouCou artwork was not copied.

## REA coverage and limitations

REA successfully bound the selected PE and reported the artifact identity. Native UI observation then reported `capability_unavailable`: its native observer requires macOS. Windows UI Automation and a selected-window PrintWindow capture supplied the runtime evidence above. The REA target was closed afterward. No native decompilation, Electron Inspector attach, transcript reading, account access or model request was performed. This is a focused interaction/reference check, not a claim of full CouCou parity.
