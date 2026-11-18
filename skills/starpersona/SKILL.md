---
name: starpersona
description: 管理星相 StarPersona 的本地 Camoufox 环境：查询、创建、修改、启动、正常关闭及归档恢复。用于星相桌面管理，不用于在网页内点击、填表或执行 JavaScript。
---

# 星相环境管理

先打开星相桌面应用。优先使用已配置的 `starpersona_*` MCP 工具；没有 MCP 时，在 StarPersona 项目根目录运行 CLI：

```sh
node scripts/starpersona.mjs status
node scripts/starpersona.mjs list
node scripts/starpersona.mjs create --json '{"name":"本地测试","os":"Windows"}'
node scripts/starpersona.mjs update <环境ID> --json '{"language":"en-US","timezone":"America/New_York"}'
node scripts/starpersona.mjs start <环境ID>
node scripts/starpersona.mjs stop <环境ID>
node scripts/starpersona.mjs archive <环境ID>
node scripts/starpersona.mjs restore <环境ID>
```

用 `list` 返回的准确 ID 操作，不凭名称猜 ID。`list --all` 包含归档；`list --query 关键词` 搜索名称或 ID。批量启动或停止可在命令后传多个 ID，逐项查看返回的 `ok` 和 `error`。

创建默认使用本地直连。可修改字段是 `name`、`os`、`language`、`timezone`、`resolution`、`groupId`、`proxyId`、`tags`、`notes`、`favorite`；分组和代理 ID 取自 `list` 的返回值。使用 `--json` 时保持有效 JSON；不要把账号密码写入参数、备注或共享配置。

`start` 返回表示进程已创建；网页是否加载成功需另行检查。`stop` 返回仅表示已请求退出，继续查询 `status`，直到该 ID 不在 `browser.instances` 中。不要强杀进程来冒充正常关闭；Linux 目前需从浏览器窗口正常退出。

运行中的环境不能修改代理或指纹启动参数，也不能归档。需要修改时先正常关闭并确认退出。归档保留浏览器数据库；复制配置不会复制登录状态。JSON 备份不包含 profile 中的浏览器数据库。

MCP / CLI 与界面共用同一个 Rust 管理器，操作结果会刷新到界面。发生“工作空间已被其他操作更新”时，重新查询并根据最新数据重试，不直接覆写文件。

扩展中心与窗口管理共用本机服务。`starpersona_extensions` 查询；`starpersona_extension_import` 导入目录或 xpi/zip；`starpersona_extension_scope` 设为 `all`（所有窗口）、`profiles`（配 `profileIds`）或 `off`。扩展代码只保留一份，各窗口的插件数据仍分开。已打开的窗口要重新打开后才加载新的范围。不要把 Chrome 商店扩展当作可加载项：当前内核是 Camoufox。移除扩展不会删除用户原来的目录，也不会清空窗口里已有的插件数据。

本接口只操作星相管理器，不提供网页快照、点击、填表、脚本执行或 CDP 端口。Camoufox 是 Firefox 系内核；网页自动化需独立的 Playwright / Juggler 适配，不能直接把 Chrome DevTools MCP 接到这些环境。详情见项目 `docs/automation.md`。
