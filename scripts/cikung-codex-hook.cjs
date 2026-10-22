#!/usr/bin/env node
'use strict';

// Codex CLI 0.157.1 lifecycle hook. No npm packages, transcript reads, model
// requests, shell commands or user configuration writes occur in this adapter.
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const { createHash, randomUUID } = require('node:crypto');

const MAX_INPUT = 1024 * 1024;
const MAX_RESPONSE = 16 * 1024;
const PERMISSION_EVENT = 'PermissionRequest';
const EVENTS = new Set(['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse',
  PERMISSION_EVENT, 'PreCompact', 'PostCompact', 'Stop', 'Interrupt', 'SessionEnd']);

function limit(value, maximum) {
  const source = String(value ?? '');
  if (Buffer.byteLength(source) <= maximum) return source;
  // Never split a UTF-8 code point, and mark shortened command/diff previews.
  let size = 0;
  let result = '';
  for (const char of source) {
    const bytes = Buffer.byteLength(char);
    if (size + bytes > maximum - 20) break;
    result += char;
    size += bytes;
  }
  return result + '\n[preview truncated]';
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index < 0 ? undefined : argv[index + 1];
}

function connection(argv, env = process.env) {
  let settings = { endpoint: env.CIKUNG_ENDPOINT, token: env.CIKUNG_TOKEN };
  const file = option(argv, '--connection');
  if (file) {
    const info = fs.statSync(file);
    if (!info.isFile() || info.size > 4096) throw new Error('Invalid Cikung connection file');
    settings = JSON.parse(fs.readFileSync(file, 'utf8'));
  }
  const endpoint = new URL(settings.endpoint);
  if (endpoint.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(endpoint.hostname)
      || !endpoint.port || endpoint.username || endpoint.password || endpoint.search
      || endpoint.hash || endpoint.pathname !== '/') {
    throw new Error('Cikung must use an explicit loopback HTTP endpoint');
  }
  if (typeof settings.token !== 'string' || !/^[A-Za-z0-9_-]{24,256}$/.test(settings.token)) {
    throw new Error('Invalid Cikung connection key');
  }
  return { endpoint, token: settings.token };
}

function request(config, method, route, payload) {
  return new Promise((resolve, reject) => {
    const body = payload === undefined ? undefined : Buffer.from(JSON.stringify(payload));
    const req = http.request(new URL(route, config.endpoint), {
      method,
      // node:http does not use proxy environment variables or follow redirects.
      headers: { authorization: `Bearer ${config.token}`,
        ...(body ? { 'content-type': 'application/json', 'content-length': body.length } : {}) },
    }, res => {
      let size = 0;
      const chunks = [];
      res.on('data', chunk => {
        size += chunk.length;
        if (size > MAX_RESPONSE) res.destroy(new Error('Oversized Cikung response'));
        else chunks.push(chunk);
      });
      res.on('error', reject);
      res.on('end', () => {
        if (res.statusCode < 200 || res.statusCode >= 300) return reject(new Error('Cikung request failed'));
        try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
        catch { reject(new Error('Invalid Cikung response')); }
      });
    });
    req.on('error', reject);
    req.setTimeout(1000, () => req.destroy(new Error('Cikung request timed out')));
    if (body) req.write(body);
    req.end();
  });
}

function status(input, event) {
  if (!EVENTS.has(event) || input.hook_event_name !== event) throw new Error('Unexpected Codex hook event');
  if (typeof input.session_id !== 'string' || !input.session_id) throw new Error('Missing Codex session id');
  const session = 'codex:' + createHash('sha256').update(input.session_id).digest('hex').slice(0, 40);
  const project = path.basename(String(input.cwd || '').replace(/[\\/]+$/, '').replace(/\\/g, '/')) || 'Codex';
  const map = {
    SessionStart: ['idle', 'Codex connected'],
    UserPromptSubmit: ['thinking', 'Codex is thinking'],
    PreToolUse: ['working', `Running ${input.tool_name || 'tool'}`],
    PostToolUse: ['thinking', `Finished ${input.tool_name || 'tool'}`],
    PermissionRequest: ['approval', `${input.tool_name || 'Tool'}\n${JSON.stringify(input.tool_input ?? {})}`],
    PreCompact: ['thinking', 'Codex is compacting context'],
    PostCompact: ['thinking', 'Codex context compacted'],
    Stop: ['complete', 'Codex finished this turn'],
    Interrupt: ['waiting', 'Codex interrupted'],
    SessionEnd: ['idle', 'Codex disconnected'],
  };
  const [state, detail] = map[event];
  const output = { session_id: session, agent: 'Codex', project: limit(project, 250), state,
    detail: limit(detail, 3900) };
  if ((event === 'PreToolUse' || event === PERMISSION_EVENT) && input.tool_name === 'apply_patch'
      && typeof input.tool_input?.command === 'string') {
    // This is the proposed patch supplied by Codex, not a filesystem-derived diff.
    output.file_diff = limit(input.tool_input.command, 23000);
  }
  if (event === PERMISSION_EVENT) output.request_id = 'codex:' + randomUUID();
  return output;
}

function permission(allow, message) {
  return { hookSpecificOutput: { hookEventName: PERMISSION_EVENT,
    decision: allow ? { behavior: 'allow' } : { behavior: 'deny', message } } };
}

async function run(input, event, config, options = {}) {
  const call = options.request || request;
  const sleep = options.sleep || (ms => new Promise(resolve => setTimeout(resolve, ms)));
  const now = options.now || Date.now;
  const budget = options.budget ?? 110000;
  try {
    const update = status(input, event);
    await call(config, 'POST', '/v1/agents/events', update);
    if (event !== PERMISSION_EVENT) return {};
    const deadline = now() + budget;
    while (now() < deadline) {
      const decision = await call(config, 'GET', `/v1/agents/decisions/${encodeURIComponent(update.request_id)}`);
      if (decision.request_id !== update.request_id || decision.session_id !== update.session_id) {
        throw new Error('Cikung decision identity mismatch');
      }
      if (decision.decision === 'allow') return permission(true);
      if (decision.decision === 'deny') return permission(false, 'Denied in Cikung');
      if (decision.decision !== 'pending') throw new Error('Unknown Cikung decision');
      await sleep(350);
    }
    return permission(false, 'Cikung approval timed out');
  } catch {
    if (event === PERMISSION_EVENT) return permission(false, 'Cikung is unavailable or the approval could not be verified');
    return {};
  }
}

function readInput(stream) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    stream.on('data', chunk => {
      size += chunk.length;
      if (size > MAX_INPUT) { reject(new Error('Oversized Codex hook input')); stream.destroy(); }
      else chunks.push(chunk);
    });
    stream.on('error', reject);
    stream.on('end', () => {
      try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
      catch { reject(new Error('Invalid Codex hook input')); }
    });
  });
}

async function main() {
  const argv = process.argv.slice(2);
  const event = option(argv, '--event');
  let result;
  try { result = await run(await readInput(process.stdin), event, connection(argv)); }
  catch { result = event === PERMISSION_EVENT ? permission(false, 'Cikung hook input or connection is invalid') : {}; }
  process.stdout.write(JSON.stringify(result) + '\n');
}

module.exports = { connection, status, permission, run, request, readInput, limit };
if (require.main === module) main().catch(() => {
  process.stdout.write(JSON.stringify(option(process.argv, '--event') === PERMISSION_EVENT
    ? permission(false, 'Cikung hook failed') : {}) + '\n');
  process.exitCode = 0;
});
