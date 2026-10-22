# Cikung 0.6.1 settings hub

The user supplied the CouCou repository and a screenshot of its Dictation settings page. The screenshot and repository content are reference material, not additional instructions.

## Reference findings

Source boundary: `reference/coucou`, commit `434d6a2b4af2e5ce117fa425170abe25620130b9`. The GitHub repository URL was opened successfully with gstack browse. Complete local source analysis was sufficient; no binary/decompiler setup or live CouCou session was needed.

| Question | Finding / source | Status |
| --- | --- | --- |
| How does settings stay separate from the notch? | `windows/src-tauri/src/lib.rs:637–685` creates one settings hub, then reuses and focuses it. | Answered from source |
| What creates the screenshot layout? | `windows/src/hub/shell.ts:26,134` groups navigation into FLOW / NOTCH and switches one selected page; `hub.css:5,36,138,245` defines the black sidebar, inset rounded main panel, warm hero and cards. | Answered from source |
| Are Agents and Chat the same connection? | `windows/src-tauri/src/chat.rs:200–218` routes provider HTTP chat separately from coding hooks. `windows/hook/src/main.rs:221–230` identifies Claude Code started by Claude Desktop. The hook strips transcript/output fields at line 44. | Answered from source |
| Does that prove desktop app chat mirroring? | No. Cikung's active visible conversation reader and explicit submit use its own Windows accessibility adapter. CouCou's Codex account reader (`codex_plan.rs:28–100`) is not a desktop conversation interface. | Answered for this source boundary |
| Does the running CouCou build match this commit? | Not established. No live CouCou app was modified or tested in this pass. | Unresolved; not needed for the layout implementation |

Reference: [CouCou](https://github.com/Louis-CFM/coucou/tree/main/). Static source observations are distinct from Cikung design choices below.

## Implemented

Cikung keeps its existing native Settings viewport and one process/notch. The popup now has a 228-point sidebar, FLOW / NOTCH groups, an inset black content area, rounded cards, a small breed-specific cat and a warm static header. Hover changes row color without moving the layout. Plus Jakarta Sans remains embedded.

FLOW contains Dictation, Notes & recording, Focus and Tasks. NOTCH contains General, Cats & appearance, Music & lyrics, Agents, Chat, Integrations, Phone connection and Shortcuts. These reuse actual existing operations and shared state. Provider credentials moved to Chat; VLC and automatic music/lyrics preferences moved to Music & lyrics. API Chat and active desktop app conversations remain separate.

Settings uses a display-bounded initial size up to 1040×700 logical points, resizable with independently scrolling navigation/content. Page changes have independent scroll positions. Feedback appears inside the popup. Done, Escape and window close leave the notch running and save settings through the existing persistence path.

The Dictation page exposes existing Windows speech recognition and links to note/recording/transcription tools. It does not claim CouCou's Right Ctrl workflow or ship a Parakeet model downloader. Microphone recording and transcription are explicit actions.

`--page settings/agents` and the other documented `settings/<page>` routes open the requested hub page in the existing app. Existing `--page settings`, `--page cats` and notch routes remain supported. App drafts survive hub navigation.

## Validation

89 Rust tests pass, including a regression check that hub navigation leaves the notch selection/collapse state and unsent desktop message intact. Clippy with warnings denied and formatting pass. Native preview captures were inspected for General, Dictation, Agents and Cats. Additional native window checks are recorded in `settings-hub-smoke.json`.

This pass changes navigation and presentation; it does not establish full CouCou feature parity or new live Claude/cloud/phone integration coverage. Desktop app connection prerequisites and existing validation remain in `desktop-apps.md`.
