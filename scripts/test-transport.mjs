import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const source = readFileSync(new URL('../src/sync/transport.ts', import.meta.url), 'utf8');
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const { fetchWithTimeout } = await import('data:text/javascript;base64,' + Buffer.from(js).toString('base64'));
const realFetch = globalThis.fetch;
try {
  globalThis.fetch = async () => new Response('{"ok":true}', { headers: { 'content-type': 'application/json' } });
  assert.deepEqual(await (await fetchWithTimeout('https://synthetic.invalid')).json(), { ok: true });
  const stalled = async (_, { signal }) => {
    if (signal.aborted) throw signal.reason;
    return await new Promise((_, reject) => signal.addEventListener('abort', () => reject(signal.reason), { once: true }));
  };
  globalThis.fetch = stalled;
  await assert.rejects(fetchWithTimeout('https://synthetic.invalid', undefined, 15), /timed out/);
  const controller = new AbortController(); controller.abort(new Error('caller cancelled'));
  await assert.rejects(fetchWithTimeout('https://synthetic.invalid', { signal: controller.signal }), /caller cancelled/);
  // Headers can arrive while JSON body stalls. The same bound must cover both.
  globalThis.fetch = async (_, { signal }) => ({ status: 200, arrayBuffer: () => stalled(null, { signal }) });
  await assert.rejects(fetchWithTimeout('https://synthetic.invalid', undefined, 15), /timed out/);
  globalThis.fetch = async () => new Response(null, { status: 204 });
  assert.equal((await fetchWithTimeout('https://synthetic.invalid')).status, 204);
  console.log('通过：网络响应、请求超时、正文超时、调用方取消与无正文响应。');
} finally { globalThis.fetch = realFetch; }
