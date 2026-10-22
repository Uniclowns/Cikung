# Cikung and Codex

Cikung connects to a **Codex CLI session launched from Cikung** using Codex's native lifecycle hooks. It can show thinking/tool activity, finished turns, interruptions, and approval requests. It does not scan saved chats or attach itself to existing Codex desktop chats or unrelated terminals.

## Start and trust the connection

1. Install the official Codex CLI and Node.js. Sign in through Codex's own normal flow.
2. In Cikung's Agents panel, choose Codex, a project folder, and the installed executables, then launch the connected terminal.
3. Inside Codex, run `/hooks`. Review and trust the Cikung command hooks. Codex skips untrusted hooks; merely opening a terminal does not mean activity callbacks have connected.
4. Work in that terminal normally. When Codex itself needs permission, Cikung shows the requested tool input. Approve once or deny there. Other configured hook policies still apply: another hook's denial wins.

Cikung does not select a model, submit a prompt, sign in, change a sandbox, or enable an approval bypass. Your Codex configuration and managed policy continue to control those choices. If hooks are disabled by user or administrator policy, the connector cannot force them on. Close the launched terminal to end that session; quitting Cikung makes subsequent approval callbacks deny when the adapter can run.

## Implementation contract

`src/agent_connect/codex.rs` exports:

- `HOOK_SOURCE`: the embedded dependency-free Node adapter.
- `REVIEW_NOTICE`: the required hook review notice for the launch UI.
- `launch_args(node, hook_script, connection_file)`: invocation-only Codex arguments, passed as separate argument values, never a joined shell command.

The launcher writes the adapter and a private, temporary connection file. The latter contains exactly:

```json
{"endpoint":"http://127.0.0.1:PORT","token":"THE_CURRENT_CIKUNG_BRIDGE_KEY"}
```

The token is not put in command arguments or saved app preferences. The launcher must protect the temporary directory and file for the current user and remove them when no longer needed. Each hook command includes the file path; this also avoids relying on Codex's shell environment filtering. For manual use the adapter accepts `CIKUNG_ENDPOINT` and `CIKUNG_TOKEN` instead.

For each supported event, the launcher adds one native TOML override, for example:

```toml
hooks.PermissionRequest = [{hooks = [{type = "command", command = '"C:\Program Files\nodejs\node.exe" "C:\private\cikung-codex-hook.cjs" --connection "C:\private\connection.json" --event PermissionRequest', timeout = 115}]}]
```

These are `codex -c key=value` invocation settings. Native Codex hook discovery reads the session-flags layer separately, alongside user/project/managed hook sources. It does not replace or rewrite their files. Cikung deliberately does not set `features.hooks`, `approval_policy`, `sandbox_mode`, `shell_environment_policy`, or `--dangerously-bypass-hook-trust`.

| Native Codex event | Cikung state |
| --- | --- |
| SessionStart | idle, connected |
| UserPromptSubmit | thinking |
| PreToolUse | working, tool name |
| PostToolUse | thinking, tool finished |
| PermissionRequest | waiting for one approval |
| PreCompact / PostCompact | thinking, context compaction |
| Stop | complete |
| Interrupt | waiting, interrupted |
| SessionEnd | idle, disconnected |

Ordinary activity requests finish quickly and have a three-second native hook timeout. Permission requests have a 115-second hook timeout; the adapter polls for at most 110 seconds. It checks both the session and request identity before accepting a decision. A denial, expired decision, malformed response, connection error, or mismatched identity returns the documented Codex deny shape:

```json
{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Reason"}}}
```

Only an explicit matching `allow` from Cikung produces `{"behavior":"allow"}`. No persistent allow rule or session-wide permission amendment is generated.

The adapter forwards the pending tool's provided input for approval review. It forwards a proposed `apply_patch` payload as a patch preview when present, bounded to 23 KB; this is **not** a filesystem-derived diff. Tool details are bounded to 3.9 KB and shortened previews are marked. Prompts and final assistant replies are not copied into Cikung. Transcript paths, authentication files, and saved-session files are never opened.

Only explicit loopback addresses (`127.0.0.1` or `::1`) with an HTTP port are accepted. No proxy, redirects, remote host, shell, model API, or third-party package is used by the adapter.

## Evidence and validation

Verified locally against **codex-cli 0.157.1**. The installed CLI's own `app-server generate-json-schema` and `generate-ts` commands exposed lifecycle and approval event definitions. The matching open-source tag `rust-v0.157.1` resolves to commit `36650394c5b38c2990ccf2a3457165ca3e9d9726`.

Relevant primary sources:

- [Official Codex hooks guide](https://developers.openai.com/codex/hooks/): inline configuration, trust, event input, permission responses, and supported tool paths.
- [Release-matched hook discovery](https://github.com/openai/codex/blob/36650394c5b38c2990ccf2a3457165ca3e9d9726/codex-rs/hooks/src/engine/discovery.rs): session-flags hook discovery and source merging.
- [Release-matched permission hook implementation](https://github.com/openai/codex/blob/36650394c5b38c2990ccf2a3457165ca3e9d9726/codex-rs/hooks/src/events/permission_request.rs): any deny wins; otherwise native allow or normal Codex approval flow.
- [Release-matched command runner](https://github.com/openai/codex/blob/36650394c5b38c2990ccf2a3457165ca3e9d9726/codex-rs/hooks/src/engine/command_runner.rs): session environment snapshot and platform command shell behavior.

Run `node --test scripts/cikung-codex-hook.test.cjs` for lifecycle, actual authenticated loopback transport, approval allow/deny, error/expiry, identity, URL validation, malformed input, and UTF-8 bounds tests. These fixtures use synthetic session IDs and no model calls.

Limitations: no live paid Codex turn was started during development. A user must trust and exercise the hooks in their own session to validate their installed policy and configuration. If the hook process itself cannot start or Codex kills it before it returns JSON, Codex controls the fallback behavior; this adapter cannot guarantee a denial outside its own running process. Hosted tools that do not emit native local-tool hooks are not reported. Older Codex versions may only have completion notifications and are not advertised as fully supported by this connector.
