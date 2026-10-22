'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { spawn } = require('node:child_process');
const path = require('node:path');
const { Readable } = require('node:stream');
const adapter = require('../adapters/cikung-claude.cjs');
const adapterPath = path.resolve(__dirname, '../adapters/cikung-claude.cjs');
const token = 'test_token_0123456789abcdef';
const base = { hook_event_name: 'PermissionRequest', session_id: 'fixture-session', cwd: 'D:/Fixture/猫',
  tool_name: 'Bash', tool_input: { command: 'echo hello', token: 'private-value' } };

async function bridge(decide = 'allow') {
  const events = [];
  const server = http.createServer(async (req, res) => {
    assert.equal(req.headers.authorization, `Bearer ${token}`);
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    res.setHeader('Content-Type', 'application/json');
    if (req.method === 'POST') {
      events.push(JSON.parse(Buffer.concat(chunks).toString('utf8')));
      res.writeHead(202); res.end('{"accepted":true}');
    } else {
      const last = events.findLast(e => e.request_id);
      const reply = { request_id: last.request_id, session_id: last.session_id, decision: decide };
      if (decide === 'wrong-id') { reply.request_id = 'other'; reply.decision = 'allow'; }
      res.end(JSON.stringify(reply));
    }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { events, env: { CIKUNG_ENDPOINT: `http://127.0.0.1:${server.address().port}`, CIKUNG_TOKEN: token },
    close: () => new Promise(resolve => server.close(resolve)) };
}

async function child(input, eventName, env) {
  return new Promise((resolve, reject) => {
    const proc = spawn(process.execPath, [adapterPath, '--event', eventName], {
      env: { ...process.env, CIKUNG_ENDPOINT: '', CIKUNG_TOKEN: '', ...env }, windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '', stderr = '';
    proc.stdout.setEncoding('utf8'); proc.stderr.setEncoding('utf8');
    proc.stdout.on('data', x => { stdout += x; }); proc.stderr.on('data', x => { stderr += x; });
    proc.on('error', reject); proc.stdin.on('error', () => {});
    proc.on('close', code => resolve({ code, stdout, stderr }));
    proc.stdin.end(typeof input === 'string' ? input : JSON.stringify(input));
  });
}
const behavior = result => JSON.parse(result.stdout).hookSpecificOutput.decision.behavior;

test('settings cover every supported hook and quote Windows apostrophes without interpolation', () => {
  const settings = adapter.buildSettings({ node: 'C:\\Program Files\\node.exe', adapter: "D:\\O'Brien\\cat.cjs", platform: 'win32' });
  assert.deepEqual(Object.keys(settings.hooks), adapter.EVENTS);
  const hook = settings.hooks.PermissionRequest[0].hooks[0];
  assert.equal(hook.shell, 'powershell'); assert.equal(hook.timeout, 115);
  assert.equal(hook.command, "& 'C:\\Program Files\\node.exe' 'D:\\O''Brien\\cat.cjs' --event 'PermissionRequest'");
  assert.equal(settings.permissions, undefined);
  assert.throws(() => adapter.buildSettings({ node: 'bad\npath', adapter: 'file' }));
});

test('POSIX settings protect paths containing shell punctuation', () => {
  const hook = adapter.buildSettings({ node: '/usr/bin/node', adapter: "/tmp/a'b;$(x).cjs", platform: 'linux' }).hooks.Stop[0].hooks[0];
  assert.equal(hook.command, "'/usr/bin/node' '/tmp/a'\"'\"'b;$(x).cjs' --event 'Stop'");
  assert.equal(hook.shell, undefined);
});

test('actual child delivers all activity states without prompts or transcripts', async () => {
  const server = await bridge();
  try {
    for (const event of adapter.EVENTS.filter(e => e !== 'PermissionRequest')) {
      const result = await child({ ...base, hook_event_name: event, prompt: 'PRIVATE PROMPT',
        transcript_path: 'PRIVATE TRANSCRIPT', message: 'PRIVATE NOTIFICATION' }, event, server.env);
      assert.equal(result.code, 0); assert.equal(result.stdout, ''); assert.equal(result.stderr, '');
    }
    assert.equal(server.events.length, 8);
    assert.deepEqual(server.events.map(e => e.state), ['working', 'thinking', 'working', 'working', 'error', 'waiting', 'complete', 'idle']);
    assert.doesNotMatch(JSON.stringify(server.events), /PRIVATE|private-value/);
  } finally { await server.close(); }
});

for (const verdict of ['allow', 'deny']) {
  test(`actual child returns explicit ${verdict} only for matching request identity`, async () => {
    const server = await bridge(verdict);
    try {
      const result = await child(base, 'PermissionRequest', server.env);
      assert.equal(result.code, 0); assert.equal(behavior(result), verdict);
      assert.equal(server.events[0].state, 'approval');
      assert.match(server.events[0].request_id, /^[a-f0-9]{32}$/);
      assert.equal(server.events[0].project, base.cwd);
      assert.doesNotMatch(JSON.stringify(server.events), /private-value/);
      const decision = JSON.parse(result.stdout).hookSpecificOutput.decision;
      assert.equal(decision.updatedPermissions, undefined);
    } finally { await server.close(); }
  });
}

test('an unrelated allow cannot approve this request', async () => {
  const server = await bridge('wrong-id');
  try { assert.equal(behavior(await child(base, 'PermissionRequest', server.env)), 'deny'); }
  finally { await server.close(); }
});

test('a pending request times out to deny and withdraws the approval', async () => {
  const server = await bridge('pending'); let stdout = '';
  try {
    await adapter.run({ input: Readable.from([JSON.stringify(base)]), output: { write: x => { stdout += x; } },
      error: { write() {} }, env: server.env, eventName: 'PermissionRequest', timeoutMs: 500 });
    assert.equal(behavior({ stdout }), 'deny'); assert.equal(server.events.at(-1).state, 'waiting');
  } finally { await server.close(); }
});

test('missing credentials, unreachable bridge, and remote endpoints cannot approve', async () => {
  for (const env of [{}, { CIKUNG_ENDPOINT: 'http://127.0.0.1:1', CIKUNG_TOKEN: token },
    { CIKUNG_ENDPOINT: 'https://example.com:443', CIKUNG_TOKEN: token },
    { CIKUNG_ENDPOINT: 'http://user:pass@127.0.0.1:1234', CIKUNG_TOKEN: token }]) {
    const result = await child(base, 'PermissionRequest', env);
    assert.equal(behavior(result), 'deny'); assert.doesNotMatch(result.stderr, /pass|test_token|example/);
  }
});

test('malformed, invalid UTF8 and oversized input still explicitly deny permission hooks', async () => {
  for (const input of ['{', JSON.stringify({ ...base, hook_event_name: 'Stop' }),
    JSON.stringify({ ...base, session_id: '../bad' }), JSON.stringify({ ...base, prompt: 'x'.repeat(129 * 1024) })]) {
    assert.equal(behavior(await child(input, 'PermissionRequest', {})), 'deny');
  }
  let stdout = '';
  await adapter.run({ input: Readable.from([Buffer.from([0xff, 0xfe])]), output: { write: x => { stdout += x; } },
    error: { write() {} }, env: {}, eventName: 'PermissionRequest' });
  assert.equal(behavior({ stdout }), 'deny');
});

test('permission notification does not supersede a pending decision', async () => {
  const server = await bridge();
  try {
    const result = await child({ ...base, hook_event_name: 'Notification', notification_type: 'permission_prompt' }, 'Notification', server.env);
    assert.equal(result.stdout, ''); assert.equal(server.events.length, 0);
  } finally { await server.close(); }
});

test('previews redact common credentials and bound unicode fields by bytes', () => {
  const command = 'API_KEY=abc TOKEN="secret words" tool --password p@ss Bearer abcd https://user:pass@example.test';
  const event = adapter.makeEvent({ ...base, cwd: '猫'.repeat(300), tool_input: { command, file_path: '猫'.repeat(1500) } }, 'PermissionRequest');
  assert.doesNotMatch(event.detail, /abc|secret words|p@ss|abcd|user:pass/);
  assert.ok(Buffer.byteLength(event.project) <= 256); assert.ok(Buffer.byteLength(event.detail) <= 3900);
  assert.ok(!event.project.includes('\ufffd'));
});

test('Edit preview uses hook strings and never opens user files', () => {
  const event = adapter.makeEvent({ ...base, tool_name: 'Edit', tool_input: {
    file_path: 'does-not-exist.txt', old_string: 'token=private', new_string: 'hello 猫',
  } }, 'PermissionRequest');
  assert.match(event.file_diff, /hello 猫/); assert.doesNotMatch(event.file_diff, /private/);
  assert.equal(adapter.makeEvent({ ...base, tool_name: 'Write', tool_input: { content: 'PRIVATE FILE' } }, 'PostToolUse').file_diff, undefined);
});
