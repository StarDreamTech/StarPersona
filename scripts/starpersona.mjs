#!/usr/bin/env node
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';
import { request } from './control-client.mjs';

const help = `StarPersona CLI — 先打开星相桌面应用
  status
  list [--query 关键词] [--all]
  create --json '{"name":"本地环境","os":"Windows"}'
  update <环境ID> --json '{"language":"en-US"}'
  start <环境ID> [更多ID...]
  stop <环境ID> [更多ID...]
  archive <环境ID>
  restore <环境ID>
  extensions
  extension-add <目录或 xpi/zip>
  extension-scope <扩展ID> --json '{"scope":"all"}'
  extension-remove <扩展ID>
  mcp          启动 stdio MCP 服务
  skill        输出 Skill 路径
所有普通命令输出 JSON；出错时退出码为 1。stop 表示请求退出，须用 status 确认完成。`;

try {
  const { values, positionals } = parseArgs({ allowPositionals: true, options: {
    json: { type: 'string' }, query: { type: 'string' }, all: { type: 'boolean' }, help: { type: 'boolean' },
  } });
  const [command, ...ids] = positionals;
  if (values.help || !command || command === 'help') console.log(help);
  else if (command === 'mcp') await import('./starpersona-mcp.mjs');
  else if (command === 'skill') console.log(fileURLToPath(new URL('../skills/starpersona/SKILL.md', import.meta.url)));
  else {
    let result;
    const data = () => {
      if (!values.json) throw new Error('此命令需要 --json 参数');
      const value = JSON.parse(values.json);
      if (!value || Array.isArray(value) || typeof value !== 'object') throw new Error('--json 应为 JSON 对象');
      return value;
    };
    const id = () => { if (ids.length !== 1) throw new Error('请指定一个环境 ID'); return ids[0]; };
    switch (command) {
      case 'status': result = await request('status'); break;
      case 'list': result = await request('profile.list', { query: values.query || '', includeArchived: !!values.all }); break;
      case 'create': result = await request('profile.create', data()); break;
      case 'update': result = await request('profile.update', { ...data(), profileId: id() }); break;
      case 'archive': case 'restore': result = await request('profile.archive', { profileId: id(), archived: command === 'archive' }); break;
      case 'extensions': result = await request('extension.list'); break;
      case 'extension-add': {
        if (ids.length !== 1) throw new Error('请指定一个扩展目录或安装包路径');
        result = await request('extension.import', { path: ids[0] });
        break;
      }
      case 'extension-scope': result = await request('extension.scope', { ...data(), id: id() }); break;
      case 'extension-remove': result = await request('extension.remove', { id: id() }); break;
      case 'start': case 'stop': {
        if (!ids.length) throw new Error('至少指定一个环境 ID');
        const results = [];
        for (const profileId of ids) {
          try { results.push({ profileId, ok: true, result: await request(`profile.${command}`, { profileId }) }); }
          catch (error) { results.push({ profileId, ok: false, error: error.message }); process.exitCode = 1; }
        }
        result = { results }; break;
      }
      default: throw new Error(`未知命令 ${command}；使用 --help 查看用法`);
    }
    console.log(JSON.stringify(result, null, 2));
  }
} catch (error) {
  console.error(JSON.stringify({ error: error.message }));
  process.exitCode = 1;
}
