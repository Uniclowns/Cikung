# Cikung 0.7: Bluetooth and game assistant

## Bluetooth

Settings → General → Bluetooth notifications enables connection monitoring on Windows. It is on by default. The connected-device list is read locally; Cikung never pairs, disconnects, or changes the radio. Existing devices establish a quiet baseline at startup or after re-enabling monitoring.

The background worker queries Windows Bluetooth Classic and LE connection selectors every three seconds. A change needs two matching successful snapshots, so announcements usually arrive about 3–6 seconds after a stable change. Duplicate snapshots and brief disconnections are quiet. Failed/timeout snapshots are shown as errors; they do not create fake disconnect events. Each enumeration has a five-second deadline.

Confirmed changes use Cikung's existing silent Windows notification path and a temporary notch header message. The notch does not expand or take focus just because a device changed. Windows notification policies can suppress the system balloon. Turning monitoring off stops the worker and clears the displayed snapshot; re-enabling starts a new baseline.

`--bluetooth-probe` prints current connected device names for diagnosis; device identifiers are excluded. Device/driver reporting varies, and a controller using a proprietary USB dongle is not necessarily a Bluetooth device.

Microsoft references: [Classic connection selector](https://learn.microsoft.com/en-us/uwp/api/windows.devices.bluetooth.bluetoothdevice.getdeviceselectorfromconnectionstatus), [LE connection selector](https://learn.microsoft.com/en-us/uwp/api/windows.devices.bluetooth.bluetoothledevice.getdeviceselectorfromconnectionstatus), [DeviceInformation enumeration](https://learn.microsoft.com/en-us/uwp/api/windows.devices.enumeration.deviceinformation.findallasync).

## Game assistant

Open More → Game, the Game card in Overview, or Settings → FLOW → Game assistant. Enter the game/version and describe the quest, location or puzzle. Choose a gentle hint, next step or full solution. The default asks the model to avoid story spoilers and the full solution.

Ask for a hint sends only the supplied context to the AI provider configured in Settings → Chat. Local models use the existing local server; cloud models need your own provider setup. An app subscription does not configure a provider API key automatically. Replies appear in the Game panel, separately from ordinary chat. Cancel discards the eventual reply; changing the context/provider also prevents a stale reply from replacing the current hint.

Claude App / Codex–ChatGPT App buttons are an alternative: they prepare the same question in the existing Agents page. Review it and press Send there. They never overwrite an existing Cikung desktop draft, automatically submit a prompt, or copy app credentials. Desktop accessibility requirements remain in `desktop-apps.md`.

This is a manual-context assistant for noncompetitive play. It does not inspect game memory, inject controls, read the screen, listen to the microphone, or claim live game-state knowledge. Suggestions depend on the model's knowledge; spoiler preference is an instruction to that model, not a guarantee. Automatic screen context and voice assistance are possible later features, not part of this release.

## Validation boundary

96 Rust tests and warnings-denied Clippy pass. New checks cover silent Bluetooth startup, stable transitions, brief-disconnect suppression, failure recovery, game context/spoiler defaults, existing-draft preservation, stale replies, and game/chat separation. Native Game hub and notch captures were inspected. The real Windows Bluetooth query succeeds and currently reports no connected devices; the Intel Bluetooth adapter is present. No physical reconnect, live model prompt, game process interaction, or microphone operation was performed.
