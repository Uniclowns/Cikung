# Connect Oh My Pi to Cikung

Cikung supports **Oh My Pi (OMP)** through a session-scoped TypeScript extension. The adapter was checked against the installed `@oh-my-pi/pi-coding-agent` **18.11.0**. It does not need a separate model key; OMP keeps using its own configured provider and login.

## Start a connected session

In Cikung Agents, select Oh My Pi, choose the project directory and start the connected terminal. Cikung supplies these environment variables only to the launched process:

- `CIKUNG_ENDPOINT`: Cikung's current HTTP loopback address and port.
- `CIKUNG_TOKEN`: the current temporary bridge key.
- `CIKUNG_PROJECT`: optional project label.
- `CIKUNG_OMP_APPROVALS`: `0` for activity monitoring, or `1` for the additional Cikung tool gate.

The launch command is `omp --extension <absolute path to adapters/cikung-omp.ts>`. Paths are separate process arguments. The adapter uses only runtime environment variables and does not save the bridge key or edit global OMP configuration. Restarting Cikung changes its connection key and requires starting a newly connected OMP session.

Already running OMP sessions do not acquire an extension automatically. Start a connected session, or load this extension using OMP's documented extension mechanism with the scoped environment above.

## What the cat shows

The extension forwards OMP's actual `session_start`, `agent_start`, `message_update`, `tool_call`, `tool_result`, `agent_end` and shutdown events. It shows idle, preparing/thinking, writing, the current tool name, failures, background waiting, and completed work. Session and subagent identifiers are separate, valid bridge identifiers.

Activity monitoring does not forward prompts, reasoning text, assistant response text, full tool arguments, tool results, or OMP login credentials. The project label and tool names are local metadata. Activity reporting failures do not stop a session in monitoring mode.

## Optional: ask Cikung before every tool

Turn on **Ask Cikung before each OMP tool** before starting the connected session. Every tool then waits for **Allow once** or **Deny** in Cikung, including read tools. Allow applies to that specific request only. Common credential-named fields in the argument preview are redacted; this is not a general secret scanner. Very long argument previews are marked as truncated so the full arguments can be reviewed in OMP.

This is an **additional gate**. OMP's own approval settings, denies, provider checks, and terminal prompts still apply afterward. When OMP itself requests permission, Cikung displays “Approve … inside OMP”; answer that prompt inside OMP. The extension never passes `--auto-approve`, never selects `yolo`, never rewrites tool arguments, and cannot grant a native OMP permission remotely.

Cikung waits at most **25 seconds per tool**, including time queued behind another tool. A denial, unknown or mismatched decision, lost connection, shutdown, or timeout blocks the tool. This stays below OMP 18.11.0's default 30-second extension-handler limit. Parallel permission requests from one session are serialized; status updates cannot invalidate an open Cikung permission. If OMP cancels an operation, its own extension runner remains responsible for preventing execution; the adapter's bounded pending check may remain visible briefly until cancellation or expiry reaches Cikung.

This gate handles OMP's model-issued `tool_call` extension events. Manually entered terminal commands and third-party paths that do not emit that event are outside its coverage. The extension does not intercept keystrokes or execute commands on its own.

## Evidence and validation

Inspected installed package source, rather than inferring behavior from CLI flag names:

- `package.json`: package identity and version 18.11.0.
- `examples/sdk/06-extensions.ts`: default extension factory and `--extension` loading.
- `src/extensibility/extensions/types.ts`: lifecycle events, tool events, `ExtensionContext.sessionManager.getSessionId()`, `ctx.agent`, and native approval notification events.
- `src/extensibility/shared-events.ts`: `tool_call` can return `{ block: true, reason }`; an undefined result continues into the host's normal checks.
- `src/extensibility/extensions/wrapper.ts`: extension preflight precedes the native approval gate; native checks are still enforced.
- `src/extensibility/extensions/runner.ts`: default extension-handler timeout is 30,000 ms, and host cancellation blocks execution.

These files were read from `C:/Users/pet/.bun/install/global/node_modules/@oh-my-pi/pi-coding-agent/`. No saved OMP conversations, provider credentials, or user configuration were read or modified.

Run the isolated fixture with `bun test scripts/test-omp.test.ts`. It exercises the shipped adapter callbacks against an authenticated loopback HTTP server, including permit/deny, timeout, disconnection, concurrent tools, shutdown, redaction and bridge limits. No real model session or paid provider request was started during validation. Live integration with an authenticated OMP model still requires a user-started connected session.
