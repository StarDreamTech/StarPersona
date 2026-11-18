# 本机浏览器验收

日期：2026-11-18。平台：Apple Silicon macOS。内核：`/Applications/Camoufox.app`，`159.0-beta.42`。桌面管理器：Tauri 2 + Rust + Vue 3。

## 真实内核集成测试：通过

使用四个临时独立 profile、一份自建 Firefox 测试插件代码和 `127.0.0.1` HTTP 服务。四个浏览器同时运行，随后逐个正常退出并重新启动。

| 环境 | 实际 platform | 语言 | 时区 | 屏幕尺寸 |
| --- | --- | --- | --- | --- |
| A | Win32 | en-US | America/New_York | 1920 × 1080 |
| B | MacIntel | ja-JP | Asia/Tokyo | 1440 × 900 |
| C | Linux x86_64 | zh-CN | Asia/Shanghai | 1920 × 1080 |
| D | Win32 | en-GB | Europe/London | 1920 × 1080 |

- 四个进程的 Cookie、localStorage、插件 storage.local 标记与扩展 UUID 各自独立。
- 四个环境重启后 Cookie、localStorage、插件标记及扩展 UUID 保持原值，插件启动计数均由 1 变成 2。
- 一份共享插件源目录；四个 profile 中没有该插件的安装目录或 XPI 副本。
- 重复启动同一环境被拒绝；运行中修改启动参数被拒绝。
- 测试进程全部正常退出，临时目录自动清理；未使用用户默认浏览器 profile。

命令：

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml real_camoufox -- --ignored --nocapture
```

结果：1 个集成测试通过，包含四环境并发及四次重启。详细本机记录在 `.artifacts/camoufox-integration.log` 和 `.artifacts/camoufox-evidence.json`（不提交 Git）。

## 常规检查

- 前端类型检查与生产构建通过。
- 前端测试 4 项通过。
- Rust 测试 8 项通过；真实浏览器测试默认忽略，已单独运行通过。
- Node 控制接口与 MCP 协议测试通过，包括分段 Unicode 响应、SDK 初始化、工具发现、参数校验与错误回传。

## 桌面与真实 AI 接口：通过

- 新版桌面自动检测本机 Camoufox，显示真实版本。
- 用官方 MCP SDK 客户端调用 7 个工具，完成创建、查询、修改、启动、关闭与归档。
- CLI 真实启动、恢复与归档通过；列表与 PID 来自同一个桌面管理器。
- 界面自动显示 MCP 创建的环境及运行状态；界面「打开 / 关闭」启动和正常退出真实内核。
- 用户在浏览器中退出后，界面自动恢复「未启动」。
- 环境运行时按 `⌘Q`，星相保持打开并提示先关闭浏览器。默认原生 Quit 菜单曾绕过退出保护，已替换为受检查的退出菜单并实测通过。
- 运行中重复启动、修改启动参数与归档均被拒绝。
- 本机 Unix socket 为 `0600`，没有开放 TCP 端口。
- 测试创建的「AI接口验收 · 本地直连」已归档保留，可以恢复使用；当前无测试浏览器进程运行。用户原有 Camoufox 未退出。

记录：`.artifacts/ai-control-evidence.json`。MCP / CLI / Skill 目前只提供管理操作，网页自动化尚未实现。

## 已知边界

- 2026-11-18 已补四环境指纹、重启、有界面模式和 CreepJS 检查：基础参数一致，但 Canvas 重启变化，Windows / Linux 仍报告 Apple GPU，当前完整设备指纹验收未通过。详细证据见 [协议与指纹实测](protocol-and-fingerprint-audit.md)。
- 已使用 WorkBuddy 的 40 个无认证 SOCKS5 节点实测：最终浏览器 HTTP/HTTPS 出口检测各 38 个成功，23 个不同 HTTPS 出口；未观察到公网 WebRTC 候选或代理失效回退直连。节点出口重复、变化及检测站访问异常见 [代理实测报告](proxy-audit.md)。
- DNS 已确认目标域名通过 SOCKS5 交给代理；完整 DNS 泄漏、GeoIP / 时区地区匹配仍未验收。独立 HTTP CONNECT 代理及代理认证未实测；目前产品只支持无认证、域名或 IPv4。
- 共享插件测试使用自建插件；实际用户插件的权限、业务功能、IndexedDB 及云端同步仍需分别验证。
- Windows / Linux 的平台构建与实际行为未验收。Linux 需手动在浏览器中正常退出；macOS 已改用针对自己托管 PID 的正常退出请求。
- 工作空间 JSON 备份不包含浏览器数据库或共享插件代码。
- 应用尚未签名、公证。
