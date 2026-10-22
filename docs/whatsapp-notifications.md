# WhatsApp notifications — Cikung 0.9.0

Settings → Notifications has the WhatsApp / WhatsApp Desk reader. The Windows version enables it by default for this requested integration; sender and message previews start **off**. Only new Windows alerts are mirrored in the Cikung header for about seven seconds. Hover over that header to see its full preview when previews are enabled. Existing Windows notifications are quiet at startup, when enabling the reader, and when changing preview visibility.

## Setup

1. Enable desktop/Windows notifications inside WhatsApp and WhatsApp Desk, and allow those apps to send notifications in Windows.
2. Open Cikung Settings → Notifications. Keep **Show WhatsApp notifications in Cikung** enabled.
3. The status should read **Connected to Windows notifications**. If Windows requires permission, use **Allow Windows access**. If access was denied, **Windows notification access** opens the relevant Windows Settings page, where permission can be restored.
4. Enable **Show sender and message preview** if wanted. Otherwise the notch displays **WhatsApp · New notification**.
5. **Test notch notification** checks Cikung's display with explicitly labeled sample text. It does not send a WhatsApp message and does not test either app's delivery to Windows.

Cikung does not open chats, reply, dismiss notifications or mark messages as read. It does not scan chat windows, browser tabs or WhatsApp databases. Previews remain in a short in-memory queue, are never saved with Cikung's profile, and are not forwarded to cloud providers, agents or the phone bridge. Turning off the reader clears its current notices. Captures, demo mode and native audit mode disable live notification reads.

## Behavior and limits

The Windows `UserNotificationListener` is checked once a second on a background thread. It verifies permission before and after reading the current notification list, filters by the official WhatsApp app identity or the exact WhatsApp / WhatsApp Desk application display name, and tracks notification ID plus creation time. The first successful snapshot is a silent baseline. Revocation drops pending messages and regrant creates a fresh baseline. Message text is read only for new matching alerts when previews are enabled. No duplicate Windows notification is emitted.

The official installed app identity observed here is `5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App`. Exact WhatsApp Desk application display names are accepted, including `WhatsAppDesk_2`. The portable executable's name alone does not establish its notification sender identity; it must publish Windows alerts under a recognized application name. A browser's WhatsApp Web notification whose source is Chrome, Edge or Firefox is outside this version. Notifications shown only inside an app, tray balloons not published to Windows Notification Center, or alerts suppressed by that app cannot be collected by this API. Fullscreen tuck-away in General still applies to the notch.

Windows permission and deployment policies vary. The native reader in this portable build was verified as **Allowed** on this PC; it does not assume every Windows installation grants access. The settings page exposes blocked/unavailable states instead of claiming a connection. Microsoft documents a notification-listener capability for packaged distribution; a future signed installer must declare it. This change does not install certificates, register a package or bypass Windows permission.

## Verification

114 Rust tests and warnings-denied Clippy passed before packaging. Regressions cover both source filters, silent startup, notification replacement/duplicates, revocation/regrant, hidden and bounded Unicode previews, profile defaults, settings routing and disabling live reads in preview mode. A native metadata-only probe returned Allowed and zero current WhatsApp notifications. Settings screenshots and the native hub route are checked separately. No real WhatsApp messages were sent to others; live arrival from the two actual apps remains unverified.

For read-only diagnostics, run `Cikung.exe --notifications-probe`. It prints only permission status and the matching notification count, without titles, senders, contents or IDs.

Primary references checked via gstack browse:

- [Microsoft notification listener guide](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener)
- [UserNotificationListener API](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.management.usernotificationlistener?view=winrt-26100)
- [Windows Settings URI reference](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings-app)
