import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { request } from './control-client.mjs';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

test('local control preserves split Unicode responses and MCP handles tools and errors', async t => {
  if (process.platform === 'win32') return t.skip('Unix socket transport');
  const dir = await mkdtemp(join(tmpdir(), 'star-control-'));
  const path = join(dir, 'control.sock');
  const seen = [];
  const server = createServer(socket => {
    socket.setEncoding('utf8');
    let input = '';
    socket.on('data', chunk => {
      input += chunk;
      if (!input.endsWith('\n')) return;
      const message = JSON.parse(input); seen.push(message);
      const response = message.params?.profileId === 'missing'
        ? { error: '环境不存在' }
        : { result: { name: '星相环境', method: message.method } };
      const bytes = Buffer.from(`${JSON.stringify(response)}\n`);
      const start = bytes.indexOf(Buffer.from('星')) + 1;
      socket.write(bytes.subarray(0, start));
      setImmediate(() => socket.end(bytes.subarray(start)));
    });
  });
  server.listen(path); await once(server, 'listening');
  t.after(async () => { await new Promise(resolve => server.close(resolve)); await rm(dir, { recursive: true, force: true }); });
  assert.equal((await request('status', {}, path)).name, '星相环境');
  await assert.rejects(request('profile.start', { profileId: 'missing' }, path), /环境不存在/);
  const client = new Client({ name: 'starpersona-test', version: '1.0.0' });
  t.after(() => client.close());
  await client.connect(new StdioClientTransport({
    command: process.execPath,
    args: [fileURLToPath(new URL('./starpersona-mcp.mjs', import.meta.url))],
    env: { ...process.env, STARPERSONA_SOCKET: path },
  }));
  const tools = await client.listTools();
  assert.deepEqual(tools.tools.map(t => t.name).sort(), [
    'status','profiles','profile_create','profile_update','profile_start','profile_stop','profile_archive',
    'extensions','extension_import','extension_scope','extension_remove',
  ].map(name => `starpersona_${name}`).sort());
  const options = {name:'启动配置测试', platform:'Portal',accountLabel:'QA',exclusiveAccount:true,startUrls:['https://example.org/']};
  await client.callTool({name:'starpersona_profile_create',arguments:options});
  assert.deepEqual(seen.at(-1), {method:'profile.create', params:options});
  assert.equal(tools.tools.find(tool => tool.name === 'starpersona_profiles').annotations.readOnlyHint, true);
  const result = await client.callTool({ name: 'starpersona_profiles', arguments: { query: '星相' } });
  assert.equal(result.structuredContent.result.name, '星相环境');
  assert.deepEqual(seen.at(-1), { method: 'profile.list', params: { query: '星相' } });
  const failed = await client.callTool({ name: 'starpersona_profile_start', arguments: { profileId: 'missing' } });
  assert.equal(failed.isError, true);
  const before = seen.length;
  const invalid = await client.callTool({ name: 'starpersona_profile_create', arguments: { name: '' } });
  assert.equal(invalid.isError, true);
  assert.equal(seen.length, before);
});
