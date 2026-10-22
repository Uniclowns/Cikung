/**
 * Oh My Pi 18.11 extension. Loaded only for a session explicitly started with -e.
 * No model calls, credential discovery, global settings writes, or policy overrides.
 */
import { createHash, randomUUID } from "node:crypto";
import { basename } from "node:path";

type Context = {
  cwd: string;
  sessionManager: { getSessionId(): string };
  agent?: { kind: string; id: string };
};
type Event = {
  toolName?: string;
  toolCallId?: string;
  input?: Record<string, unknown>;
  isError?: boolean;
  approved?: boolean;
  willContinue?: boolean;
  awaitingAsyncWork?: boolean;
  assistantMessageEvent?: { type?: string };
};
type Block = { block: true; reason: string };
type API = { on(name: string, handler: (event: Event, ctx: Context) => Promise<void | Block>): void };
type State = "idle" | "working" | "thinking" | "waiting" | "approval" | "complete" | "error";
type Payload = {
  session_id: string; agent: string; project: string; state: State;
  detail: string; request_id?: string;
};
type Options = {
  env?: Record<string, string | undefined>;
  /** Test seam; production stays below OMP's 30 second handler timeout. */
  approvalTimeoutMs?: number;
  pollIntervalMs?: number;
};
type Session = { tail: Promise<unknown>; network: Promise<void>; pending: boolean; queued: number; phase: string; ended: boolean };

function clip(value: string, bytes: number): string {
  const data = Buffer.from(value);
  return data.length <= bytes ? value : data.subarray(0, bytes - 4).toString("utf8").replace(/\uFFFD$/, "") + "…";
}

export function loopbackEndpoint(value: string | undefined): string {
  if (!value) throw new Error("Cikung endpoint is missing. Start OMP from Cikung.");
  const url = new URL(value);
  if (url.protocol !== "http:" || !["127.0.0.1", "[::1]"].includes(url.hostname)
      || !url.port || url.username || url.password || url.search || url.hash || !["", "/"].includes(url.pathname)) {
    throw new Error("Cikung requires an HTTP loopback endpoint with an explicit port.");
  }
  return url.origin;
}

function inputPreview(input: Record<string, unknown> | undefined): string {
  if (!input) return "";
  // Keep approval context local and hide common credential fields. No tool output is uploaded.
  const text = JSON.stringify(input, (key, value) =>
    /(?:token|password|secret|authorization|api[_-]?key|credential)/i.test(key) ? "[redacted]" : value,
  );
  if (Buffer.byteLength(text ?? "") > 2800) {
    return clip(text ?? "", 2700) + "\n[Preview truncated; review full tool arguments in OMP]";
  }
  return text ?? "";
}

/** Exported to exercise the real extension callbacks against a local fixture. */
export function installCikungOMP(api: API, options: Options = {}): void {
  const env = options.env ?? process.env;
  const endpoint = loopbackEndpoint(env.CIKUNG_ENDPOINT);
  const token = env.CIKUNG_TOKEN;
  if (!token || token.length > 256 || /[^\x21-\x7e]/.test(token)) {
    throw new Error("Cikung connection key is missing or invalid. Start OMP from Cikung.");
  }
  const requireApproval = env.CIKUNG_OMP_APPROVALS === "1";
  const timeout = Math.min(25_000, Math.max(50, options.approvalTimeoutMs ?? 25_000));
  const pollEvery = Math.max(10, options.pollIntervalMs ?? 350);
  const instance = randomUUID();
  const sessions = new Map<string, Session>();
  let closed = false;

  const identity = (ctx: Context): string => {
    // Native IDs can contain arbitrary Unicode. Hashing keeps bridge IDs stable and valid.
    const raw = `${instance}:${ctx.sessionManager.getSessionId()}:${ctx.agent?.id ?? "main"}`;
    return "omp-" + createHash("sha256").update(raw).digest("hex").slice(0, 48);
  };
  const session = (id: string): Session => {
    let value = sessions.get(id);
    if (!value) {
      if (sessions.size >= 32) {
        const old = [...sessions].find(([, entry]) => entry.ended && !entry.pending && entry.queued === 0);
        if (old) sessions.delete(old[0]);
        else throw new Error("Cikung OMP session limit reached.");
      }
      value = { tail: Promise.resolve(), network: Promise.resolve(), pending: false, queued: 0, phase: "", ended: false };
      sessions.set(id, value);
    }
    return value;
  };
  const call = async (path: string, body?: Payload, deadline = Date.now() + 2000): Promise<unknown> => {
    const response = await fetch(endpoint + path, {
      method: body ? "POST" : "GET",
      headers: { Authorization: `Bearer ${token}`, ...(body ? { "Content-Type": "application/json" } : {}) },
      body: body ? JSON.stringify(body) : undefined,
      redirect: "error",
      signal: AbortSignal.timeout(Math.max(1, Math.min(2000, deadline - Date.now()))),
    });
    if (!response.ok) { await response.body?.cancel(); throw new Error("Cikung connection unavailable."); }
    if (!response.body) throw new Error("Cikung returned an empty response.");
    const reader = response.body.getReader();
    const parts: Uint8Array[] = [];
    let size = 0;
    try {
      for (;;) {
        const chunk = await reader.read();
        if (chunk.done) break;
        size += chunk.value.length;
        if (size > 4096) throw new Error("Cikung response exceeded the limit.");
        parts.push(chunk.value);
      }
      return JSON.parse(Buffer.concat(parts).toString("utf8"));
    } finally { await reader.cancel().catch(() => {}); }
  };
  const payload = (ctx: Context, state: State, detail: string, request?: string): Payload => ({
    session_id: identity(ctx), agent: "Oh My Pi",
    project: clip(env.CIKUNG_PROJECT || basename(ctx.cwd) || ctx.cwd, 240),
    state, detail: clip(detail, 3800), ...(request ? { request_id: request } : {}),
  });
  const publish = async (ctx: Context, state: State, detail: string): Promise<void> => {
    if (closed) return;
    const current = session(identity(ctx));
    // Any normal bridge event invalidates its session's pending approval. Never race it.
    current.network = current.network.then(async () => {
      if (closed || current.pending) return;
      const phase = state + ":" + detail;
      if (current.phase === phase) return;
      current.phase = phase;
      try { await call("/v1/agents/events", payload(ctx, state, detail)); }
      catch { current.phase = ""; /* Monitoring must not crash the coding agent. */ }
    });
    await current.network;
  };
  const block = (reason: string): Block => ({ block: true, reason });

  api.on("session_start", async (_, ctx) => { await publish(ctx, "idle", "OMP connected"); });
  api.on("session_switch", async (_, ctx) => { await publish(ctx, "idle", "OMP session connected"); });
  api.on("agent_start", async (_, ctx) => {
    session(identity(ctx)).ended = false;
    await publish(ctx, "thinking", "Preparing response");
  });
  api.on("message_update", async (event, ctx) => {
    const kind = event.assistantMessageEvent?.type;
    if (kind === "thinking_start") await publish(ctx, "thinking", "Thinking");
    else if (kind === "text_start") await publish(ctx, "working", "Writing response");
  });
  api.on("tool_call", async (event, ctx) => {
    const tool = clip(event.toolName || "tool", 120);
    if (!requireApproval) { await publish(ctx, "working", `Tool: ${tool}`); return; }
    const current = session(identity(ctx));
    if (closed || current.queued >= 32) return block("Cikung is unavailable or has too many pending tool calls.");
    const deadline = Date.now() + timeout;
    current.queued++;
    const previous = current.tail;
    let release!: () => void;
    const reservation = new Promise<void>(resolve => { release = resolve; });
    current.tail = previous.then(() => reservation, () => reservation);
    let ownsGate = false;
    let allowed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      // Bound queue wait as well as polling: OMP's handler timeout remains authoritative.
      const ready = await Promise.race([
        previous.then(() => true, () => true),
        new Promise<boolean>(resolve => { timer = setTimeout(() => resolve(false), timeout); timer.unref?.(); }),
      ]);
      if (timer) clearTimeout(timer);
      if (!ready || closed || Date.now() >= deadline) return block("Cikung approval timed out; tool was blocked.");
      current.pending = true;
      ownsGate = true;
      await current.network;
      const request = "omp-" + randomUUID();
      const description = `Allow this OMP tool once? ${tool}\n${inputPreview(event.input)}`;
      await call("/v1/agents/events", payload(ctx, "approval", description, request), deadline);
      while (!closed && Date.now() < deadline) {
        const result = await call("/v1/agents/decisions/" + request, undefined, deadline) as {
          decision?: string; session_id?: string; request_id?: string;
        };
        if (result.session_id !== identity(ctx) || result.request_id !== request) {
          return block("Cikung returned a mismatched permission; tool was blocked.");
        }
        if (result.decision === "allow") { allowed = true; return; }
        if (result.decision === "deny") return block("Tool denied in Cikung.");
        if (result.decision !== "pending") return block("Cikung returned an invalid permission; tool was blocked.");
        await new Promise(resolve => setTimeout(resolve, Math.min(pollEvery, Math.max(1, deadline - Date.now()))));
      }
      return block("Cikung approval timed out or disconnected; tool was blocked.");
    } catch {
      return block("Cikung connection failed; tool was blocked. Reconnect Cikung and retry.");
    } finally {
      current.queued--;
      if (timer) clearTimeout(timer);
      if (ownsGate) {
        current.pending = false;
        current.phase = "";
        // Release the next gate only after clearing the previous pending permission.
        await publish(ctx, allowed ? "waiting" : "error", allowed
          ? "Cikung allowed once; OMP native policy still applies" : "Cikung blocked the tool");
      }
      release();
    }
  });
  api.on("tool_approval_requested", async (event, ctx) => {
    await publish(ctx, "waiting", `Approve ${event.toolName || "tool"} inside OMP`);
  });
  api.on("tool_approval_resolved", async (event, ctx) => {
    await publish(ctx, event.approved ? "working" : "error",
      `OMP ${event.approved ? "approved" : "denied"}: ${event.toolName || "tool"}`);
  });
  api.on("tool_result", async (event, ctx) => {
    await publish(ctx, event.isError ? "error" : "working",
      `${event.isError ? "Failed" : "Finished"}: ${event.toolName || "tool"}`);
  });
  api.on("agent_end", async (event, ctx) => {
    const ongoing = event.willContinue || event.awaitingAsyncWork;
    session(identity(ctx)).ended = !ongoing;
    await publish(ctx, ongoing ? "waiting" : "complete", ongoing ? "Waiting for background work" : "OMP finished");
  });
  api.on("session_shutdown", async (_, ctx) => {
    // Invalidate the pending permission before closing; pending callbacks then fail closed.
    const current = session(identity(ctx));
    current.pending = false;
    await publish(ctx, "idle", "OMP disconnected");
    closed = true;
  });
}

export default function cikungOMP(api: API): void { installCikungOMP(api); }
