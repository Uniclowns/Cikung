# Google Calendar

Cikung adds tasks that have a due time to your Google Calendar: each one becomes a 30-minute event in your primary calendar. Editing a task moves its event, and deleting the task or clearing its due time removes it. Finished tasks keep their event.

Tasks can come from the Tasks page or from the chat assistant: ask it something like *"remind me about the client meeting tomorrow at 3 pm"* and it shows a card you confirm with **Add task**.

Cikung signs in with **your own** Google Cloud OAuth client, so no one else's app ever holds access to your calendar. It asks only for the `calendar.events` scope. The sign-in token is kept in the system credential store (Windows Credential Manager / macOS Keychain), never in Cikung's settings file.

## One-time setup (about 10 minutes)

1. Open [Google Cloud Console](https://console.cloud.google.com/) and create a project, for example **Cikung**.
2. Go to **APIs & Services → Library**, search for **Google Calendar API** and press **Enable**.
3. Go to **Google Auth Platform** (formerly *OAuth consent screen*) and press **Get started**:
   - App name: **Cikung**, and your email as the support email.
   - Audience: **External**.
   - Contact email: your email. Accept the policy and press **Create**.
4. Under **Audience**, press **Publish app** and confirm, so the status reads **In production**. While an app is in *Testing*, Google ends its sign-ins after 7 days.
5. Under **Clients**, press **Create client**:
   - Application type: **Desktop app**, name **Cikung**.
   - Press **Create**, then copy the **Client ID** and **Client secret**.
6. In Cikung, open **Settings → Tasks → Google Calendar**, paste both, and press **Connect Google**.
7. In the browser, choose your Google account. Google warns that it **hasn't verified this app**: that is expected for a personal client. Press **Advanced → Go to Cikung (unsafe)**, then allow access to your calendar events.
8. The tab says *Cikung is connected to Google Calendar*. Close it; Cikung confirms in the notch.

## Notes

- Cikung signs in through a temporary page on `127.0.0.1` on this PC, protected with PKCE. The sign-in waits five minutes.
- **Disconnect Google** forgets the sign-in on this PC and asks Google to revoke it. You can also remove Cikung under *Third-party apps & services* in your Google account.
- Without a connection, Cikung retries two minutes after a failure; tasks waiting to sync go up once it is back.
- Events Cikung already made are not changed if you turn the sync off; new and edited tasks simply stay local.
