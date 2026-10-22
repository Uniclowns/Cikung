#!/usr/bin/env node
'use strict';

// Claude Code command hook. This adapter never executes tool_input or reads a transcript.
const http = require('node:http');
const crypto = require('node:crypto');
const { performance } = require('node:perf_hooks');
const { TextDecoder } = require('node:util');

const EVENTS = Object.freeze(['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse',
  'PostToolUseFailure', 'PermissionRequest', 'Notification', 'Stop', 'SessionEnd']);
const MAX_INPUT = 128 * 1024;
const MAX_RESPONSE = 16 * 1024;
const ID = /^[A-Za-z0-9_.:-]{1,128}$/;

function bounded(value, bytes) {
  let text = typeof value === 'string' ? value : '';
  if (Buffer.byteLength(text, 'utf8') <= bytes) return text;
  // Iterating code points cannot leave a partial UTF-8 sequence or surrogate pair.
  let result = '', length = 0;
  for (const point of text) {
    const size = Buffer.byteLength(point);
    if (length + size > bytes - 3) break;
    result += point; length += size;
  }
  return result + '…';
}

function redact(value) {
  return String(value)
    .replace(/\b(Bearer\s+)[A-Za-z0-9._~+\/-]+/gi, '$1[redacted]')
    .replace(/((?:api[_-]?key|access[_-]?token|refresh[_-]?token|token|password|passwd|secret|authorization|credential)[\w-]*\s*[=:]\s*)(?:"[^"\r\n]*"|'[^'\r\n]*'|[^\s;&,}]+)/gi, '$1[redacted]')
    .replace(/(--(?:api[-_]?key|token|password|secret|credential)\s+)(?:"[^"\r\n]*"|'[^'\r\n]*'|[^\s;&]+)/gi, '$1[redacted]')
    .replace(/(https?:\/\/)[^/@\s]+:[^/@\s]+@/gi, '$1[redacted]@');
}

function preview(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return '';
  // Only the fields a user needs to review a tool request, never full MCP arguments.
  const fields = ['command', 'file_path', 'path', 'url', 'pattern', 'description'];
  const lines = [];
  for (const key of fields) {
    if (typeof input[key] === 'string') lines.push(`${key}: ${bounded(redact(input[key]), 2200)}`);
  }
  if (!lines.length) lines.push('Review the full tool arguments in the Claude terminal.');
  return bounded(lines.join('\n'), 3100);
}

function fileDiff(hook) {
  // No file reads: these are the Edit changes Claude already supplied to the hook.
  if (hook.tool_name !== 'Edit' || !hook.tool_input || typeof hook.tool_input !== 'object') return undefined;
  const input = hook.tool_input;
  if (typeof input.old_string !== 'string' || typeof input.new_string !== 'string') return undefined;
  return bounded(`--- ${redact(input.file_path || 'file')}\n+++ ${redact(input.file_path || 'file')}\n${redact(input.old_string).split('\n').map(x => '- ' + x).join('\n')}\n${redact(input.new_string).split('\n').map(x => '+ ' + x).join('\n')}`, 12000);
}

function connection(env) {
  const address = new URL(env.CIKUNG_ENDPOINT || '');
  if (address.protocol !== 'http:' || address.hostname !== '127.0.0.1' || !address.port ||
    address.pathname !== '/' || address.search || address.hash || address.username || address.password) {
    throw new Error('Invalid local bridge address');
  }
  const token = env.CIKUNG_TOKEN || '';
  if (!/^[A-Za-z0-9_-]{16,256}$/.test(token)) throw new Error('Missing or invalid bridge credential');
  return { address, token };
}

function request(conn, method, path, body, timeout = 2500) {
  return new Promise((resolve, reject) => {
    const bytes = body === undefined ? undefined : Buffer.from(JSON.stringify(body), 'utf8');
    const req = http.request({ hostname: '127.0.0.1', port: conn.address.port, path, method,
      headers: { Authorization: `Bearer ${conn.token}`, ...(bytes ? {
        'Content-Type': 'application/json', 'Content-Length': bytes.length,
      } : {}) }, agent: false }, response => {
      const chunks = []; let length = 0;
      response.on('data', chunk => {
        length += chunk.length;
        if (length > MAX_RESPONSE) { response.destroy(); reject(new Error('Bridge response too large')); }
        else chunks.push(chunk);
      });
      response.on('error', reject);
      response.on('end', () => {
        if (response.statusCode < 200 || response.statusCode >= 300) return reject(new Error('Bridge rejected event'));
        try { resolve(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks)))); }
        catch { reject(new Error('Invalid bridge response')); }
      });
    });
    const timer = setTimeout(() => req.destroy(new Error('Bridge request timed out')), timeout);
    req.on('close', () => clearTimeout(timer));
    req.on('error', reject);
    req.end(bytes);
  });
}

async function readInput(stream) {
  const chunks = []; let length = 0;
  for await (const chunk of stream) {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    length += bytes.length;
    if (length > MAX_INPUT) throw new Error('Hook input too large');
    chunks.push(bytes);
  }
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks)));
}

function decision(allow) {
  return { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: allow ? {
    behavior: 'allow',
  } : { behavior: 'deny', message: 'Cikung did not receive an explicit approval. Review and retry in Claude Code.' } } };
}

function makeEvent(hook, eventName) {
  if (!ID.test(hook.session_id || '')) throw new Error('Invalid session identifier');
  const tool = bounded(redact(hook.tool_name || 'tool'), 100);
  const mapping = {
    SessionStart: ['working', 'Claude session connected'],
    UserPromptSubmit: ['thinking', 'Working on your request'],
    PreToolUse: ['working', `Using ${tool}`], PostToolUse: ['working', `Finished ${tool}`],
    PostToolUseFailure: ['error', `${tool} failed; see Claude terminal`],
    PermissionRequest: ['approval', `Allow ${tool}?\n${preview(hook.tool_input)}`],
    Notification: ['waiting', 'Claude needs your attention; see terminal'],
    Stop: ['complete', 'Claude finished this turn'], SessionEnd: ['idle', 'Claude session ended'],
  };
  const state = mapping[eventName];
  if (!state) throw new Error('Unsupported event');
  return { session_id: hook.session_id, agent: 'Claude Code',
    project: bounded(redact(hook.cwd || ''), 256), state: state[0], detail: bounded(state[1], 3900),
    ...(eventName === 'PermissionRequest' ? { request_id: crypto.randomUUID().replace(/-/g, '') } : {}),
    ...(['PermissionRequest', 'PostToolUse'].includes(eventName) && fileDiff(hook) ? { file_diff: fileDiff(hook) } : {}),
  };
}

async function run({ input = process.stdin, output = process.stdout, error = process.stderr,
  env = process.env, eventName, timeoutMs = 110000 } = {}) {
  // The event argument comes from our generated settings, not the untrusted stdin.
  const permission = eventName === 'PermissionRequest';
  let conn, payload, published = false, allowed = false;
  try {
    if (!EVENTS.includes(eventName)) throw new Error('Pass the configured --event name');
    const hook = await readInput(input);
    if (!hook || typeof hook !== 'object' || hook.hook_event_name !== eventName) throw new Error('Hook event mismatch');
    // Claude emits this notification beside PermissionRequest. Publishing it would supersede
    // the pending decision on older Cikung bridges. The PermissionRequest is authoritative.
    if (eventName === 'Notification' && hook.notification_type === 'permission_prompt') return;
    conn = connection(env); payload = makeEvent(hook, eventName);
    await request(conn, 'POST', '/v1/agents/events', payload);
    published = true;
    if (permission) {
      const deadline = performance.now() + Math.min(110000, Math.max(500, timeoutMs));
      while (performance.now() < deadline) {
        const remaining = deadline - performance.now();
        const reply = await request(conn, 'GET', `/v1/agents/decisions/${payload.request_id}`, undefined,
          Math.max(1, Math.min(2500, remaining)));
        if (reply.request_id !== payload.request_id || reply.session_id !== payload.session_id) {
          throw new Error('Bridge decision identity mismatch');
        }
        if (reply.decision === 'allow') { allowed = true; break; }
        if (reply.decision === 'deny') break;
        if (reply.decision !== 'pending') throw new Error('Invalid decision');
        await new Promise(resolve => setTimeout(resolve, Math.min(600, Math.max(1, deadline - performance.now()))));
      }
    }
  } catch {
    // Never include request contents, credentials, URLs, or native errors in the hook log.
    error.write('Cikung: local agent connection unavailable or hook rejected.\n');
  } finally {
    if (permission) {
      output.write(JSON.stringify(decision(allowed)) + '\n');
      if (published && !allowed) {
        try { await request(conn, 'POST', '/v1/agents/events', {
          session_id: payload.session_id, agent: payload.agent, project: payload.project,
          state: 'waiting', detail: 'Permission denied or wait expired; retry in Claude terminal.',
        }, 500); } catch { /* Best-effort withdraw; never change a deny to allow. */ }
      }
    }
  }
}

function buildSettings({ node, adapter, platform = process.platform }) {
  const literal = value => {
    if (typeof value !== 'string' || !value || /[\0\r\n]/.test(value)) throw new Error('Invalid adapter path');
    return platform === 'win32' ? `'${value.replace(/'/g, "''")}'` : `'${value.replace(/'/g, `'"'"'`)}'`;
  };
  return { hooks: Object.fromEntries(EVENTS.map(event => [event, [{ hooks: [{ type: 'command',
    command: `${platform === 'win32' ? '& ' : ''}${literal(node)} ${literal(adapter)} --event ${literal(event)}`,
    ...(platform === 'win32' ? { shell: 'powershell' } : {}),
    timeout: event === 'PermissionRequest' ? 115 : 5,
  }] }]])) };
}

module.exports = { EVENTS, bounded, redact, preview, makeEvent, connection, decision, buildSettings, run };
if (require.main === module) {
  const i = process.argv.indexOf('--event');
  run({ eventName: i < 0 ? undefined : process.argv[i + 1] }).catch(() => { process.exitCode = 1; });
}
