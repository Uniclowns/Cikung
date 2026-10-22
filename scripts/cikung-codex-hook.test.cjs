'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { spawn } = require('node:child_process');
const { once } = require('node:events');
const path = require('node:path');
const adapter = require('./cikung-codex-hook.cjs');

const fixture = (event, extra = {}) => ({ session_id: 'fixture-session', cwd: 'C:\\Work\\Example',
  hook_event_name: event, tool_name: 'Bash', tool_input: { command: 'echo fixture' }, ...extra });
const config = () => adapter.connection([], { CIKUNG_ENDPOINT: 'http://127.0.0.1:45678',
  CIKUNG_TOKEN: 'fixture-connection-key-not-a-real-key' });

test('maps the documented lifecycle without copying prompts or assistant replies', () => {
  const cases = { SessionStart: 'idle', UserPromptSubmit: 'thinking', PreToolUse: 'working',
    PostToolUse: 'thinking', Stop: 'complete', Interrupt: 'waiting', SessionEnd: 'idle' };
  for (const [event, state] of Object.entries(cases)) {
    const update = adapter.status(fixture(event, { prompt: 'private prompt', last_assistant_message: 'private reply' }), event);
    assert.equal(update.state, state);
    assert.equal(update.agent, 'Codex');
    assert.match(update.session_id, /^codex:[a-f0-9]{40}$/);
    assert.ok(!JSON.stringify(update).includes('private'));
  }
});

test('authenticates and forwards an actual loopback event over HTTP', async t => {
  let received;
  const server = http.createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    received = { authorization: req.headers.authorization, url: req.url,
      body: JSON.parse(Buffer.concat(chunks).toString()) };
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end('{"ok":true}');
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  t.after(() => server.close());
  const settings = config();
  settings.endpoint = new URL(`http://127.0.0.1:${server.address().port}`);
  assert.deepEqual(await adapter.run(fixture('PreToolUse'), 'PreToolUse', settings), {});
  assert.equal(received.authorization, `Bearer ${settings.token}`);
  assert.equal(received.url, '/v1/agents/events');
  assert.equal(received.body.state, 'working');
});

test('returns only the documented one-request Codex permission decision', async () => {
  for (const expected of ['allow', 'deny']) {
    let event;
    const output = await adapter.run(fixture('PermissionRequest'), 'PermissionRequest', config(), {
      request: async (_config, method, _route, payload) => {
        if (method === 'POST') { event = payload; return { ok: true }; }
        return { session_id: event.session_id, request_id: event.request_id, decision: expected };
      },
    });
    assert.equal(output.hookSpecificOutput.hookEventName, 'PermissionRequest');
    assert.equal(output.hookSpecificOutput.decision.behavior, expected);
    assert.equal(output.hookSpecificOutput.decision.updatedPermissions, undefined);
    assert.equal(event.state, 'approval');
    assert.match(event.request_id, /^codex:/);
  }
});

test('bridge failure, wrong identity and approval expiry all deny', async () => {
  const requests = [
    async () => { throw new Error('offline'); },
    async () => ({ session_id: 'other', request_id: 'other', decision: 'allow' }),
  ];
  for (const request of requests) {
    const result = await adapter.run(fixture('PermissionRequest'), 'PermissionRequest', config(), { request });
    assert.equal(result.hookSpecificOutput.decision.behavior, 'deny');
  }
  let event;
  const result = await adapter.run(fixture('PermissionRequest'), 'PermissionRequest', config(), {
    budget: 0,
    request: async (_config, _method, _route, payload) => { event = payload; return {}; },
  });
  assert.equal(event.state, 'approval');
  assert.equal(result.hookSpecificOutput.decision.behavior, 'deny');
  assert.match(result.hookSpecificOutput.decision.message, /timed out/);
});

test('connection validation rejects remote, credential, and redirect-shaped URLs', () => {
  for (const endpoint of ['https://127.0.0.1:4567', 'http://example.com:4567',
    'http://localhost:4567', 'http://user:pass@127.0.0.1:4567',
    'http://127.0.0.1:4567/path', 'http://127.0.0.1:4567?secret=x']) {
    assert.throws(() => adapter.connection([], { CIKUNG_ENDPOINT: endpoint, CIKUNG_TOKEN: config().token }));
  }
});

test('CLI emits valid deny JSON for malformed approval input', async () => {
  const child = spawn(process.execPath, [path.join(__dirname, 'cikung-codex-hook.cjs'), '--event', 'PermissionRequest'],
    { stdio: ['pipe', 'pipe', 'pipe'], env: {} });
  const output = [];
  child.stdout.on('data', chunk => output.push(chunk));
  child.stdin.end('{invalid');
  const [code] = await once(child, 'exit');
  assert.equal(code, 0);
  assert.equal(JSON.parse(Buffer.concat(output).toString()).hookSpecificOutput.decision.behavior, 'deny');
});

test('bounds UTF-8 event previews and validates event identity', () => {
  const update = adapter.status(fixture('PermissionRequest', { tool_input: { command: '猫'.repeat(5000) } }), 'PermissionRequest');
  assert.ok(Buffer.byteLength(update.detail) <= 3900);
  assert.ok(update.detail.endsWith('[preview truncated]'));
  assert.throws(() => adapter.status(fixture('Stop'), 'PermissionRequest'));
});
