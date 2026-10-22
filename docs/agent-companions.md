# Cikung 0.6 — one app and a family of cats

Cikung opens one black notch in the top center of the screen. Features and characters share this one application. Opening it again brings back the existing window; it never starts a fleet of companions.

**Overview** shows each feature with its own cat. Pick Music, Agents, Focus or any other feature to open it inside the same notch. **Cats** opens the breed gallery: Orange tabby, Tuxedo, Calico, Siamese, Silver shorthair and Snow. Accessories distinguish the feature looks further.

Leave **Let each feature have its own cat** enabled for automatic feature characters. Under **Choose a cat for each feature**, change the assignment for Music, Notes, Tasks and the other tools, plus Claude App, Oh My Pi and Codex / ChatGPT App separately. The cat in the header follows the selected feature or agent. Alternatively, click a breed card to use that favorite everywhere. All choices are saved together. Ctrl+Alt+C toggles the notch; Settings changes the shortcut, display or startup preference.

Files, tasks, notes and Phone now use the same application state. Notes → Extract tasks immediately adds marked items to Tasks. The optional Phone connection sees the same agent sessions and media. Leaving the Agents page does not disconnect an agent.

On first normal launch after 0.3, Cikung merges the previous feature profiles into its shared profile. Existing workspace data is preserved; conflicting task/note IDs are reassigned. Original feature files stay intact and the old shared profile is backed up as `app.before-single-cat-v1.ron`. A migration error is displayed; it does not delete the original profiles. Legacy `--feature` selects a page; `--workspace` still opens the same single app.

## Connect desktop AI apps

Select **Claude App** or **Codex / ChatGPT App** in Agents. **Open app** focuses the installed desktop app or opens its registered Windows app link. Finish loading and sign-in there. Cikung reads the active window through Windows accessibility, shows working/ready/attention status and the visible conversation, and sends your typed message only when you press **Send to active chat**. **Link active chat** pauses or enables that app's link.

An existing draft, a busy app, an ambiguous message box, or a changed active conversation prevents sending. Approval dialogs stay in the app; Cikung does not automatically approve actions. The desktop link does not use the CLI adapters, account files, API keys, or browser debugging ports. The conversation is kept in memory only. See [Desktop app details](desktop-apps.md).

Oh My Pi retains its previous project-folder and connected-terminal integration. Its optional tool approval gate is off by default. The older Claude Code/Codex CLI adapters remain in source for compatibility and their setup guides describe that separate integration; the default Claude/Codex UI now targets desktop apps.

Phone continues to mirror the hook-based agent bridge and media; desktop accessibility conversations are currently local to the PC.

## Validation boundary

Windows x64 is the validated target. Local HTTP fixtures exercise adapter events, authentication and explicit decisions; a harmless Windows terminal fixture checks exact argument preservation. These checks do not establish successful login, model access or a paid end-to-end coding task. Native macOS/Linux placement and terminal launching still require platform-specific work and validation.
