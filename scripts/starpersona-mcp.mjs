#!/usr/bin/env node
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { request } from './control-client.mjs';

const server = new McpServer({ name: 'starpersona', version: '0.1.0' });
const fields = {
  name: z.string().trim().min(1).max(80).optional(),
  os: z.enum(['Windows', 'macOS', 'Linux']).optional(),
  language: z.enum(['zh-CN', 'en-US', 'en-GB', 'ja-JP']).optional(),
  timezone: z.enum(['Asia/Shanghai', 'America/Los_Angeles', 'America/New_York', 'Asia/Tokyo', 'Asia/Singapore', 'Europe/London', 'UTC']).optional(),
  resolution: z.enum(['1920 × 1080', '1440 × 900', '2560 × 1440', '1366 × 768']).optional(),
  groupId: z.string().max(100).optional(), proxyId: z.string().max(100).optional(),
  tags: z.array(z.string().max(30)).max(12).optional(), notes: z.string().max(1000).optional(), favorite: z.boolean().optional(),
};
const profileId = z.string().min(1).max(100).describe('先查询环境列表，再使用返回的准确 ID');
function tool(name, description, method, inputSchema, readOnly = false, destructive = false) {
  server.registerTool(name, {
    description, inputSchema,
    annotations: { readOnlyHint: readOnly, destructiveHint: destructive, openWorldHint: false },
  }, async args => {
    try {
      const result = await request(method, args);
      return { content: [{ type: 'text', text: JSON.stringify(result) }], structuredContent: { result } };
    } catch (error) {
      return { isError: true, content: [{ type: 'text', text: error.message }] };
    }
  });
}
tool('starpersona_status', '查询本机内核、真实 PID、关闭中状态和错误。只管理环境，不提供网页操作或 CDP。', 'status', {}, true);
tool('starpersona_profiles', '查询环境、分组、代理配置和运行状态。默认不含归档环境。', 'profile.list', { query: z.string().optional(), includeArchived: z.boolean().optional() }, true);
tool('starpersona_profile_create', '创建独立环境配置；默认本地直连，不复制其他环境登录数据。', 'profile.create', { ...fields, name: z.string().trim().min(1).max(80) });
tool('starpersona_profile_update', '修改环境配置。运行中的启动参数不可更改；先正常关闭。', 'profile.update', { ...fields, profileId });
tool('starpersona_profile_start', '用独立持久化目录启动本机 Camoufox。禁止同一环境重复启动。', 'profile.start', { profileId });
tool('starpersona_profile_stop', '请求正常退出指定托管进程；不强杀。返回后用 status 确认进程退出。', 'profile.stop', { profileId });
tool('starpersona_profile_archive', '归档或恢复环境配置，保留浏览器数据；运行中不可归档。', 'profile.archive', { profileId, archived: z.boolean() }, false, true);
await server.connect(new StdioServerTransport());
