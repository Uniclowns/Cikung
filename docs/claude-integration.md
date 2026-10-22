# Claude Code connection

Cikung connects to a new **Claude Code terminal session** through Claude's supported command hooks. It shows activity, Edit previews, and tool permission requests. It does not take over existing Claude Desktop conversations, read transcript files, or send prompts to a model.

The installed CLI used for discovery was `Claude Code 2.1.237`. Its `--help` documents `--settings <file-or-json>` for session settings. Hook event and permission output shapes were checked against the [official hooks reference](https://code.claude.com/docs/en/hooks) on 18 October 2026. That online reference describes some newer features; this integration deliberately does not use the newer `onFailure` or executable `args` fields.

## Session launch contract

The launcher needs the user's selected working directory, Claude executable, Node executable, and the absolute path to `adapters/cikung-claude.cjs`.

1. Create a temporary JSON settings file for this launch, outside the user's project configuration.
2. Set `CIKUNG_ENDPOINT` and `CIKUNG_TOKEN` on the launched process. The address must be the active Cikung loopback address, `http://127.0.0.1:<port>`. Never put the token in command arguments or the JSON file.
3. Launch the selected Claude executable with separate arguments `--settings` and the temporary file's absolute path, in the selected working directory. Preserve Claude's regular interactive permission checks.
4. After that session ends, the launcher may delete its own temporary settings file. Existing user and project settings are neither rewritten nor removed.

The adapter exports `buildSettings({ node, adapter, platform })` for generation and fixture validation. `platform` is `win32`, `linux`, or `darwin`. The returned object has this shape:

```json
{
  "hooks": {
    "PermissionRequest": [{
      "hooks": [{
        "type": "command",
        "command": "& 'C:\\Program Files\\nodejs\\node.exe' 'D:\\Nuch\\adapters\\cikung-claude.cjs' --event 'PermissionRequest'",
        "shell": "powershell",
        "timeout": 115
      }]
    }]
  }
}
```

Generate an entry for **each** event listed below. Other events use a five-second hook timeout. On Windows, use PowerShell literal strings: replace each single quote in a path with two single quotes, and retain the leading `&`. Reject paths with NUL/newline characters. On macOS/Linux, omit `shell`, omit `&`, and use POSIX single-quote escaping. `buildSettings` implements both forms. These command strings contain fixed adapter paths and event names, never tool commands or prompt text.

## Events and permissions

| Claude event | Cikung state |
| --- | --- |
| SessionStart | Working; session connected |
| UserPromptSubmit | Thinking |
| PreToolUse / PostToolUse | Working; tool name |
| PostToolUseFailure | Error; inspect the terminal |
| PermissionRequest | Waiting for explicit approval |
| Notification | Needs attention; inspect the terminal |
| Stop | Completed the current turn |
| SessionEnd | Idle |

The accompanying `permission_prompt` notification is ignored because the actual PermissionRequest carries its own authoritative request. The adapter publishes a fresh random request identifier and polls Cikung for that exact identifier and Claude session. Only an explicit matching `allow` produces `decision.behavior: "allow"`. Denial, malformed responses, mismatched identifiers, connection failures, and a 110-second wait expiry produce an explicit deny. An allow applies once and does not save a Claude permission rule. Preview strings are never executed.

This is an additional review channel for requests Claude actually emits. Tools already permitted by existing Claude settings may run without a PermissionRequest. Existing hooks still follow Claude's own configuration precedence. If Node cannot start, the process is externally killed, or Claude rejects the settings, the adapter cannot emit a verdict; Claude's native permission handling remains authoritative. Claude versions supporting `onFailure: "block"` can provide a stronger hook-failure boundary, but that newer option is not enabled for the installed version.

## Data handling

The adapter reads only its stdin and the two connection environment variables. It never opens the supplied `transcript_path`, accesses Claude authentication, reads source files, or modifies Claude settings. Requests go directly to loopback using Node's HTTP client without redirects or proxies.

It forwards the session ID, working directory, tool name and activity. For review, selected command/path/URL fields and bounded Edit strings may be shown. Prompts, notification messages, full tool responses and arbitrary MCP arguments are omitted. Common credential assignments, flags, bearer tokens and URL credentials are redacted. Redaction is heuristic: review previews can still contain project source or unfamiliar secret formats, so use the full Claude terminal when that context matters.

Input is capped at 128 KiB, HTTP responses at 16 KiB, project text at 256 UTF-8 bytes, detail at 3900 bytes and Edit preview at 12 KiB. Invalid UTF-8 is rejected; truncation preserves complete characters. Diagnostic errors contain no raw input, token, URL, or transport details.

## Verification

Run `node --test scripts/test-claude-adapter.cjs`. Twelve tests launch the actual adapter as a child process against an authenticated loopback fixture where appropriate. They cover all activity events, allow/deny identity matching, timeout withdrawal, bridge failure, invalid input and UTF-8, byte limits, credential redaction, Edit previews, and both platform quoting builders.

No model calls or real Claude tasks were run during this validation. A real user-started Claude session is still needed to confirm its locally installed configuration accepts and dispatches the hooks in that environment.
