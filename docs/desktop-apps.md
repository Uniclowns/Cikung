# Claude App and Codex / ChatGPT App

Cikung 0.6 uses the installed desktop apps through Windows UI Automation. This is a visible-window integration, not a Claude Code or Codex CLI session. The Windows OpenAI.Codex package on the development PC identifies its executable and Start-menu label as ChatGPT; Cikung labels the connection Codex / ChatGPT App.

Select the app in Agents and use Open app if needed. Finish loading/sign-in in that app. Link active chat enables or pauses local monitoring. A running process alone is not displayed as Connected: an accessible editable chat composer is required. Working follows the app's Stop/Stop generating control; sign-in and approval controls produce a needs-attention state. Cikung never invokes approval buttons.

The conversation panel reads bounded visible text from the active conversation area, falling back to the app's visible document if a dedicated conversation region is not exposed. It is not a full historical chat sync or background-task API. Window/app UI changes, minimized windows, loading states, inaccessible controls, or differing privilege levels can limit it. No conversation content is saved to preferences or diagnostic output. Windows app accounts remain in their own apps.

Send to active chat verifies the target process path, window/process identity and a conversation fingerprint before writing and again immediately before invoking Send. It refuses a busy app, nonempty existing draft, ambiguous composer or changed conversation. It uses accessibility Value and Invoke patterns, with no clipboard or simulated typing. If a submit button is unavailable, it removes only its own unchanged inserted draft and keeps the original message in Cikung. Messages go to the currently open conversation; Cikung does not create or switch chats automatically.

The current adapter recognizes ProseMirror/prompt-textarea/chat-input and named chat composers. Exact known submit labels include Send, Send message, Send prompt and Submit. Unsupported layouts show an unavailable/waiting state instead of guessing coordinates. Opening uses the apps' locally verified codex:// and claude:// protocol registrations. It does not edit app configuration or restart apps.

Validation includes a real Windows Forms fixture: reading its conversation, setting its composer with UI Automation, invoking Send, reading the changed reply, rejecting a changed conversation both before and after draft insertion, rolling back its own draft, and preserving an existing draft. No test prompt is sent to a real model or existing user chat. `--desktop-probe` reports connection state and visible text character counts without message contents. Live app capability depends on the app finishing loading and exposing its UI.

This integration is currently Windows-only. Oh My Pi keeps its terminal adapter. Phone mirroring still uses the separate event bridge, not desktop conversation text.

Primary OS reference: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview

The UI embeds Figtree Bold under SIL OFL 1.1, a free lookalike of Spotify's interface font. The packaged license is licenses/OFL-Figtree.txt. Battery percentage and Pin are removed; hover grace, click-to-toggle and interaction holds remain.

## Local validation evidence

On this PC, the actual Claude app exposed its ProseMirror composer and exact Send button. A temporary connection-check draft was inserted and cleared without pressing Send. The actual Codex/ChatGPT app exposed its active conversation text and Working state. Its composer was left untouched; the empty-editor hint behavior was checked separately. Existing-draft protection was exercised in the native fixture. The end-to-end submit/reply test uses the isolated native fixture, not a real user chat. This distinction avoids claiming a paid/live model exchange was tested.

The observed Codex/ChatGPT UIA provider exposes the empty-editor hint as its Value. The adapter treats the exact known hint as empty only when there is no enabled Send/Queue control; any other text, or the same text with an enabled submit control, remains a protected draft. This has a regression test.


## Completion alerts in 0.10

Linked desktop apps now arm a completion alert after an observed Working state, then require two Ready snapshots with an unchanged conversation fingerprint in the same window/process/title. Loading, closing, pausing, attention or a changed chat/window cancels the pending alert. Alerts are queued while snapshots are drained, so a busy Cikung UI cannot lose the transition. Settings → Notifications controls the embedded Windows chime. See [completion sounds](completion-sounds.md) for hook behavior and limits. The native UIA fixture now exposes a controlled Stop generating button and finish signal; it verifies the real Working → Ready detection and one-shot alert without using a real model.
