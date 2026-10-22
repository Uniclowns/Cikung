# Local agents and Phone Link

Cikung receives real agent activity over an authenticated loopback connection. It can show steps, a supplied diff, and explicit permission requests. It never executes commands, reads your agent conversations, or approves requests automatically. Events must be sent by a hook or an integration; opening another agent app alone does not provide activity.

## Connect an agent

Open **Agents → Connection setup** in Cikung. Copy the current local address and agent token. The address and token change each time Cikung starts. Keep the token private and set these variables in the same terminal from which you launch your agent:

```powershell
$env:CIKUNG_ENDPOINT = 'http://127.0.0.1:PORT'
$env:CIKUNG_TOKEN = 'COPY_THE_AGENT_TOKEN_FROM_CIKUNG'
```

Publish a real event using the included script (replace the path if you installed Cikung elsewhere):

```powershell
& 'D:\Nuch\scripts\cikung-hook.ps1' -SessionId 'my-session' -Agent 'My agent' -Project 'My project' -State working -Detail 'Reviewing the media adapter'
```

Request one explicit decision:

```powershell
& 'D:\Nuch\scripts\cikung-hook.ps1' -SessionId 'my-session' -Agent 'My agent' -State approval -Detail 'Allow this specific operation?'
```

The script prints `{"request_id":"…","decision":"allow"}` or `deny`. The caller must honor that decision. Timeout, connection failure, and superseded requests deny access. This does not bypass your agent's own permissions. A generic script failure exits with a nonzero status; your adapter must treat failure as denial.

### Claude Code

This script supports Claude Code's `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Notification`, `Stop`, and `SessionEnd` hook payloads. Permission requests return Claude Code's `hookSpecificOutput.decision.behavior` with `allow` or `deny`. Tool previews are supplied by Claude Code; Cikung does not run the tool. The preview can contain sensitive command arguments; only pair phones you trust.

Cikung does **not** edit your Claude Code or Codex configuration. To opt in, merge the following into your project's `.claude/settings.local.json`, preserving existing settings. Set `CIKUNG_ENDPOINT` and `CIKUNG_TOKEN` in the terminal before starting Claude Code. Use the actual script path; the example assumes this source checkout. The 120 second hook timeout permits the script's default 110 second decision wait. Some Claude Code environments do not emit `PermissionRequest` hooks in noninteractive modes; Cikung can only decide requests it receives.

```json
{
  "hooks": {
    "SessionStart": [{"hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}],
    "PreToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}],
    "PostToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}],
    "PermissionRequest": [{"matcher": ".*", "hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode", "timeout": 120}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}],
    "Stop": [{"hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}],
    "SessionEnd": [{"hooks": [{"type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File D:/Nuch/scripts/cikung-hook.ps1 -ClaudeCode"}]}]
  }
}
```

On macOS/Linux, use PowerShell 7 (`pwsh`) if installed, or send the same HTTP API from your agent's supported hook mechanism. There is no automatic Codex permission-hook adapter: the generic event API works where you can explicitly integrate it, and approval integration requires the caller to wait for and honor the decision. The bridge does not claim to observe every installed assistant.

### HTTP API

All local calls require `Authorization: Bearer AGENT_TOKEN`, exact `Host: 127.0.0.1:PORT`, and JSON POST bodies. No cross-origin browser access is enabled.

* `POST /v1/agents/events`: `session_id`, `agent`, `project`, `state`, `detail`, optional `file_diff`; an `approval` event requires a unique `request_id`.
* `GET /v1/agents/decisions/REQUEST_ID`: `decision` is `pending`, `allow`, or `deny`.
* `GET /v1/state`: current agent sessions and media.

States are `idle`, `working`, `thinking`, `waiting`, `approval`, `complete`, and `error`. IDs use 1–128 ASCII letters, numbers, `.`, `:`, `_`, or `-`. Requests expire after 120 seconds. Sessions are capped at 32, history at 40 steps per session, body size at 48 KiB, diff size at 24 KiB, and requests at 300 per source per minute. A newer event for the same session denies its unresolved earlier request. Decision records are retained for at most 15 minutes. Nothing is persisted across restart.

## Link your phone

1. Put your phone and computer on the same **trusted** local network.
2. In Cikung's Phone panel, enable Phone Link. This starts a separate LAN listener. Windows may ask you to allow Cikung on your private network; public network access is unnecessary.
3. Scan the displayed QR code or open its pairing URL on your phone. The link exchanges a separate random pairing key for an HttpOnly, SameSite=Strict session cookie. It never gives your phone the local agent token.
4. The phone shows real media and agent state. You can control supported media players and allow or deny pending agent requests. Disable Phone Link to revoke every paired phone and stop the LAN listener. Re-enabling creates a new link and credentials.

The LAN page uses HTTP, so traffic is **not encrypted**; use a trusted private network. The pairing link is a credential and must not be shared publicly. Cookies expire after 12 hours, there are at most eight paired browsers, and the bridge accepts only its displayed LAN IP/port as the host. Phone authentication cannot publish agent events or execute arbitrary commands. Media controls only queue `play`, `pause`, `toggle`, `next`, or `previous` for the desktop's native media adapter.

The page includes a web app manifest and mobile layout. Use Safari's **Share → Add to Home Screen**, or your browser's home-screen shortcut command when available. Browser installation prompts and offline service workers generally require HTTPS; this local HTTP companion is an online home-screen web shortcut, not a full offline PWA or a native iOS/Android app. Native Live Activities, push notifications, remote access outside your LAN, and encrypted relay hosting are not implemented.
