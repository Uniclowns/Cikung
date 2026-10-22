import { afterEach, expect, test } from "bun:test";
import { installCikungOMP, loopbackEndpoint } from "../adapters/cikung-omp";

const servers: ReturnType<typeof Bun.serve>[] = [];
afterEach(() => { for (const server of servers.splice(0)) server.stop(true); });

function fixture(approval = false, decide: (body: any) => string = () => "allow", timeout = 500) {
  const events: any[] = [];
  const decisions = new Map<string, any>();
  const headers: string[] = [];
  let delayed = false;
  const server = Bun.serve({
    hostname: "127.0.0.1", port: 0,
    async fetch(request) {
      headers.push(request.headers.get("authorization") || "");
      if (request.headers.get("authorization") !== "Bearer fixture-only") return new Response("denied", { status: 401 });
      if (request.method === "POST") {
        const body = await request.json();
        if (delayed && body.state !== "approval") await Bun.sleep(60);
        events.push(body);
        // Model the bridge's real supersession rule.
        for (const old of decisions.values()) {
          if (old.session_id === body.session_id && old.decision === "pending") old.decision = "deny";
        }
        if (body.request_id) decisions.set(body.request_id, {
          request_id: body.request_id, session_id: body.session_id, decision: decide(body),
        });
        return Response.json({ ok: true });
      }
      const id = new URL(request.url).pathname.split("/").pop()!;
      return Response.json(decisions.get(id) || { decision: "unknown" });
    },
  });
  servers.push(server);
  const handlers = new Map<string, any>();
  installCikungOMP({ on(name, handler) { handlers.set(name, handler); } }, {
    env: { CIKUNG_ENDPOINT: `http://127.0.0.1:${server.port}`, CIKUNG_TOKEN: "fixture-only", CIKUNG_OMP_APPROVALS: approval ? "1" : "0" },
    approvalTimeoutMs: timeout, pollIntervalMs: 10,
  });
  const ctx = { cwd: "D:/Project", sessionManager: { getSessionId: () => "user session / Unicode 猫" }, agent: { kind: "main", id: "Main" } };
  return { events, decisions, headers, ctx, server,
    delay() { delayed = true; },
    emit: (event: string, body: any = {}, context = ctx) => handlers.get(event)(body, context),
  };
}

test("accepts only explicit loopback ports and refuses remote credential destinations", () => {
  expect(loopbackEndpoint("http://127.0.0.1:1234/")).toBe("http://127.0.0.1:1234");
  expect(loopbackEndpoint("http://[::1]:1234")).toBe("http://[::1]:1234");
  for (const value of [undefined, "https://example.com:1234", "http://localhost:1234", "http://127.0.0.1", "http://x@127.0.0.1:1234", "http://127.0.0.1:1234/path", "http://127.0.0.1:1234?secret=x"]) {
    expect(() => loopbackEndpoint(value)).toThrow();
  }
});

test("status-only forwards genuine callbacks without prompts or model output", async () => {
  const f = fixture();
  await f.emit("session_start");
  await f.emit("agent_start");
  await f.emit("message_update", { assistantMessageEvent: { type: "thinking_start", delta: "private reasoning" } });
  expect(await f.emit("tool_call", { toolName: "read", input: { secret: "never-send" } })).toBeUndefined();
  await f.emit("tool_approval_requested", { toolName: "bash" });
  await f.emit("tool_approval_resolved", { toolName: "bash", approved: false });
  await f.emit("tool_result", { toolName: "bash", isError: true });
  await f.emit("agent_end", { willContinue: true });
  await f.emit("agent_end");
  expect(f.events.map(event => event.state)).toEqual(["idle", "thinking", "thinking", "working", "waiting", "error", "error", "waiting", "complete"]);
  expect(f.events.every(event => /^omp-[a-f0-9]{48}$/.test(event.session_id))).toBe(true);
  expect(new Set(f.events.map(event => event.session_id)).size).toBe(1);
  expect(JSON.stringify(f.events)).not.toContain("never-send");
  expect(JSON.stringify(f.events)).not.toContain("private reasoning");
  expect(f.headers.every(header => header === "Bearer fixture-only")).toBe(true);
});

test("additional approval only resumes after an explicit matched allow", async () => {
  const f = fixture(true);
  const result = await f.emit("tool_call", { toolName: "write", input: { path: "a.txt", password: "hidden" } });
  expect(result).toBeUndefined();
  expect(f.events[0].state).toBe("approval");
  expect(f.events[0].detail).toContain("[redacted]");
  expect(f.events[0].detail).not.toContain("hidden");
  expect(f.events[1].detail).toContain("native policy still applies");
});

test("deny, malformed decision, timeout and disconnected server block", async () => {
  for (const decision of ["deny", "broken", "pending"]) {
    const f = fixture(true, () => decision, 70);
    const result = await f.emit("tool_call", { toolName: "bash" });
    expect(result.block).toBe(true);
    expect(f.events.at(-1).state).toBe("error");
  }
  const f = fixture(true);
  f.server.stop(true);
  expect((await f.emit("tool_call", { toolName: "bash" })).block).toBe(true);
});

test("concurrent tool requests are serialized and status cannot supersede an approval", async () => {
  let latest = "";
  const f = fixture(true, body => { latest = body.request_id; return "pending"; }, 900);
  const first = f.emit("tool_call", { toolName: "read" });
  const second = f.emit("tool_call", { toolName: "grep" });
  await Bun.sleep(50);
  await f.emit("tool_result", { toolName: "another-tool" });
  expect(f.events.filter(event => event.state === "approval").length).toBe(1);
  f.decisions.get(latest).decision = "allow";
  expect(await first).toBeUndefined();
  await Bun.sleep(40);
  expect(f.events.filter(event => event.state === "approval").length).toBe(2);
  f.decisions.get(latest).decision = "deny";
  expect((await second).block).toBe(true);
});

test("in-flight monitoring finishes before a new permission is posted", async () => {
  const f = fixture(true);
  f.delay();
  const status = f.emit("agent_start");
  await Bun.sleep(10);
  const result = await f.emit("tool_call", { toolName: "edit" });
  await status;
  expect(result).toBeUndefined();
  expect(f.events.map(event => event.state)).toEqual(["thinking", "approval", "waiting"]);
});

test("shutdown cancels pending approval and later tool calls fail closed", async () => {
  const f = fixture(true, () => "pending", 700);
  const pending = f.emit("tool_call", { toolName: "bash" });
  await Bun.sleep(30);
  await f.emit("session_shutdown");
  expect((await pending).block).toBe(true);
  expect((await f.emit("tool_call", { toolName: "write" })).block).toBe(true);
});

test("subagent identities stay distinct and all text obeys bridge byte limits", async () => {
  const f = fixture(true);
  await f.emit("tool_call", { toolName: "猫".repeat(100), input: { text: "猫".repeat(9000) } });
  await f.emit("session_start", {}, { ...f.ctx, agent: { kind: "sub", id: "0-Explore" } });
  expect(new Set(f.events.map(event => event.session_id)).size).toBe(2);
  expect(f.events.every(event => Buffer.byteLength(event.detail) <= 4000)).toBe(true);
});
