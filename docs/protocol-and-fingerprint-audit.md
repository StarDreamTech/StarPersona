# Camoufox 协议与指纹实测

日期：2026-11-18。环境：本机 Apple Silicon macOS，Camoufox `159.0-beta.42`。结论：**CDP 不可用，WebDriver BiDi 可用；基础配置和数据隔离通过，当前管理器的配置未通过“稳定且系统一致的设备指纹”验收。**

## CDP / MCP 实测

Firefox 使用 Gecko；它曾提供部分 CDP 实现，Mozilla 从 Firefox 141 起移除了该实现。支持自动化并不要求使用 Chromium，也不表示采用 CDP。

本次分别使用临时 profile 启动 Camoufox 与 Chrome，没有附加或关闭用户已有浏览器。调试监听均在本机回环地址。正向对照为本机 Chrome `157.0.8103.79`。

| 检查 | Camoufox 159 | Chrome 157 对照 |
| --- | --- | --- |
| HTTP `/json/version`、`/json/list` | 404 | 200，返回 CDP 端点 |
| CDP `Browser.getVersion` | 建立 BiDi 会话后仍报 `unknown command` | 返回 Chrome 版本 |
| Playwright `firefox.connectOverCDP` | 明确拒绝，仅支持 Chromium | 不适用 |
| Playwright `chromium.connectOverCDP` | HTTP 404 | 成功连接 |
| Google `chrome-devtools-mcp` 1.5.0，`list_pages` | HTTP 模式报 404；直接 WebSocket 也报协议错误 | 成功列出页面 |
| 原生 WebDriver BiDi | `session.new`、页面列表、`script.evaluate` 成功 | 本次未测 |

BiDi 是另一套浏览器自动化协议。本次已证明该本机 Camoufox 能用它读取页面，但尚未把 BiDi 网页操作工具接入星相。`status.pageAutomation` 仍是 `false`；不要把协议探针误当成产品功能已经交付。

新增网页操作时，可以优先评估每环境独立 BiDi 连接，由星相 MCP 按环境 ID 路由。每环境需要在启动时开启自己独立的本机端口，管理会话和退出；不能把 BiDi WebSocket 地址直接塞给 Google CDP MCP。

证据：`.artifacts/cdp-proof.json`；本机复现脚本：`node .artifacts/cdp-proof.mjs`。脚本使用本机已缓存的 Google MCP 和 Playwright 包，不修改项目依赖。缓存路径变化时需更新脚本。

## 指纹测试方法

- 使用真实 Rust `BrowserManager`，四个临时 profile 同时运行，各自拥有不同配置 ID。
- 配置分别为 Windows/en-US/New York、macOS/ja-JP/Tokyo、Linux/zh-CN/Shanghai、Windows/en-GB/London。
- 在同一个本机 HTTP 源采集浏览器 API，并在四个环境逐个正常退出、重新打开后重复采集。
- 对比 HTTP 请求头、主页面、同源 iframe 和 Worker；绘制相同 Canvas、WebGL 图形并计算 SHA-256；测字体宽高、离线音频、RTC ICE 候选与 HTTP 出口。
- 另用星相桌面实际启动有界面的 Windows 环境，正常关闭并重启，确认无头测试中的问题也存在于普通使用流程。
- 使用独立临时环境访问 CreepJS，核对第三方页面实际显示的系统、GPU、字体和语音信息。没有读取用户默认 profile。
- IP 只在内存中比较，JSON 保存 SHA-256；第三方页面文本中的 IPv4 已脱敏。

复现四环境测试：

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml real_camoufox -- --ignored --nocapture
```

测试中的断言检查配置、跨页面上下文一致性与数据隔离。测试退出码 0 **不等于完整反检测通过**；以下设备指纹缺口由采样比较单独报告。

## 结果

| 项目 | 实测结果 | 判断 |
| --- | --- | --- |
| UA、系统标识、语言、时区、屏幕尺寸 | 四环境与所填基础参数对应；HTTP UA 与 JS UA 相同 | 基础配置通过 |
| 主页面、iframe、Worker | UA、平台、语言、时区、CPU 数一致；`webdriver=false` | 已测项目一致，不代表不可被识别 |
| Cookie、localStorage、插件数据 | 四环境各自独立，重启保留 | 数据隔离通过 |
| Canvas 同一进程重复绘制 | 同一测试图重复读取与新画布结果相同 | 会话内稳定 |
| Canvas 正常退出再重启 | 四个临时环境全部变化；有界面环境也变化，Cookie 同时仍保留 | 未满足当前环境长期保持相同设备指纹的目标 |
| WebGL / GPU | 四个环境都报告 `Apple M1, or similar`；两个 Windows 与一个 Linux 环境也如此 | 三个环境与所选系统组合不一致 |
| WebGL 图像、离线音频 | 此次样本四环境各只有一个相同哈希，重启不变 | 存在共同特征；单项同值本身不等于可判定同一设备 |
| 字体 | 字体度量重启稳定；两个 Windows 环境相同，Windows 样本仍出现多种 macOS 字体度量特征 | 尚未形成经验证的成套系统字体配置 |
| CreepJS | 读取 Windows/Win32，同时读取 Apple GPU；en-US 环境的默认语音为 `Tingting [zh-CN]` | 第三方复核了 GPU 组合与语言相关残留；未宣称取得任何风控“通行分数” |
| WebRTC | 直连时 STUN 公网候选与 HTTP 出口一致；后续 40 个真实 SOCKS5 环境均只采到 mDNS | 本次代理样本未观察到公网地址泄漏，详见 [代理补测](proxy-audit.md) |

本次不能将问题泛化为所有 Camoufox 版本或所有配置都无效。当前星相只传入基础参数，没有加载经校验、成套且持久化的完整设备配置。管理器里的 `fingerprint` 字符串目前仅作配置标识，没有作为绘图种子或设备配置传入内核。

Canvas 重启变化对于隐私随机化可以是设计选择，但对需要长期维持同一账号设备身份的环境，当前行为不满足稳定性目标。应先决定所需身份稳定策略，再选择内核支持的配置方式。

## 后续修复重点与未验收范围

1. 先约束可支持的系统与设备组合，补齐 GPU、字体、CPU、语音等成套信息及跨上下文校验。
2. 依据当前 Camoufox 的真实配置能力实现设备配置持久化，验证同环境重启与不同环境的表现，避免照抄旧教程。上游 #860 报告自 156 起已移除旧 Canvas 种子与字距噪声接口；不能直接靠添加旧版 `canvas:seed` 字段宣称修复。
3. 同日已使用 WorkBuddy 的 40 个真实 SOCKS5 节点补测出口、WebRTC/STUN、DNS 交接和代理断开行为，详见 [逐条代理报告](proxy-audit.md)。完整 DNS 泄漏、独立 HTTP CONNECT 代理和时区与 IP 地区匹配仍未验收。
4. 本轮不覆盖商业平台的登录、验证码、封禁和账号关联判定，也不覆盖多个无关站点之间的 Canvas 表现。

本轮只增加测试与文档，**没有修改生产指纹参数**。验收环境完成后已重新归档；测试浏览器与本机测试服务已关闭，用户原有 Camoufox 保持运行。浏览器日志出现过 macOS sandbox extension 警告；未禁用或绕过沙箱，协议与页面采样仍成功返回。

## 证据与官方资料

- `.artifacts/fingerprint-summary.json`：比较摘要与失败项。
- `.artifacts/fingerprint-evidence.json`：四环境与重启原始采样。
- `.artifacts/fingerprint-headed-comparison.json`：桌面正常模式的前后对比。
- `.artifacts/creepjs-proof.json`、`.artifacts/creepjs-visible.txt`：第三方页面显示内容。
- `.artifacts/fingerprint-integration.log`：真实浏览器测试输出。
- [Mozilla：Firefox 141 发布说明，移除 CDP](https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/141)
- [Mozilla 开发团队：CDP 退休与 BiDi 迁移](https://fxdx.dev/cdp-retirement-in-firefox/)
- [MDN：Gecko 引擎](https://developer.mozilla.org/en-US/docs/Glossary/Gecko)
- [Google Chrome DevTools MCP](https://github.com/ChromeDevTools/chrome-devtools-mcp)
- [Playwright BrowserType：CDP 仅适用于 Chromium](https://playwright.dev/docs/api/class-browsertype)
- [Camoufox #327：CDP 连接讨论](https://github.com/daijro/camoufox/issues/327)
- [Camoufox #860：新版本已移除旧噪声接口](https://github.com/daijro/camoufox/issues/860)
- [CreepJS 实测页面](https://abrahamjuliot.github.io/creepjs/)
