# 星相 StarPersona

<img src="public/icon.svg" width="64" height="64" alt="星相图标">

**简体中文** · [English](README.en.md)

星相是基于 [Camoufox](https://github.com/daijro/camoufox) 的开源桌面浏览器环境管理器。每个环境使用独立的数据目录。当前已验证 Apple Silicon macOS，Windows 和 Linux 尚未验收。

## 在 Mac 上打开

如果项目已经打包，在项目根目录运行：

```sh
open src-tauri/target/release/bundle/macos/StarPersona.app
```

如果刚克隆仓库，`.app` 需要先构建；也可以按下方开发步骤直接启动桌面版。第一次使用时，新建一个**本地直连**环境，点「打开」。如果界面没有找到浏览器，点「配置内核」选择 `Camoufox.app`。

**0.2 新功能：** 在「配置内核」下载官方版本，不必另装官方管理 GUI；「共享书签」统一编辑并同步到已启用的环境。在「创建窗口 → 账号与启动」设置启动网址、平台账号查重和同时打开限制。macOS 可选 Dock 环境编号。[内核实际大小、Homebrew 安装与更新、验收范围](docs/desktop-distribution.md)。

## 当前版本

基于 **Tauri 2 + Rust + Vue 3 + TypeScript** 的本地桌面管理界面。参考多账号浏览器的布局，提供星相品牌、浅蓝侧栏、环境表格和配置抽屉。

- 浏览器环境创建、编辑、复制、收藏、搜索和排序。
- 列表 / 卡片视图、分组筛选、分页、批量移动和归档恢复。
- 分组和无认证 SOCKS5 / HTTP 代理配置管理；代理列表可按名称、主机、端口、地区、协议和状态筛选。
- 操作系统、语言、时区、分辨率与独立指纹标识配置。
- Rust 校验并保存本地 JSON，写入失败保留原有工作空间。
- 原生文件对话框导入 / 导出备份；导入前校验，替换前确认。
- 工作台、系统设置、帮助、键盘快捷键和窄屏适配。
- 复用本机 Camoufox，启动 / 关闭独立进程，自动刷新真实进程状态。
- 每环境独立持久化 profile；应用基础操作系统标识、语言、时区、屏幕尺寸和代理。
- 扩展中心保存一份 Firefox 扩展代码，可加载到所有窗口或指定窗口。

**已接入本地 Camoufox 的基础启动能力。** 当前直接使用内核支持的配置参数，不需要额外的 Python 运行时。指纹标识仍只是配置 ID；尚未实现完整设备指纹生成、GeoIP 自动匹配。不要将基础接入视为完整反检测能力。

初次运行是空工作空间，不预置示例环境或代理。可以手动添加，也可以在「代理 IP → 从 StarPort 导入代理列表」导入本机代理端口。旧版本留下的示例环境和示例代理会在启动时自动清理，其他数据保留。

## 实测结论

2026-11-18，Apple Silicon macOS，Camoufox `159.0-beta.42`。以下结论只覆盖表中列出的版本和样本。

| 验收项 | 实测结果 |
| --- | --- |
| [环境与插件隔离](docs/browser-testing.md) | 四个环境同时运行，共用一份解压后的测试插件代码。Cookie、localStorage 和插件数据各自独立，重启后保留。 |
| [设备指纹](docs/protocol-and-fingerprint-audit.md) | 系统标识、语言、时区和屏幕尺寸与配置一致；四个环境的 Canvas 均在重启后变化，三个 Windows / Linux 环境都报告 Apple GPU。**未通过稳定设备指纹验收。** |
| [页面控制协议](docs/protocol-and-fingerprint-audit.md) | Camoufox 的 CDP 发现端点返回 404；WebDriver BiDi 能列页面、执行脚本。Chrome 157 对照能通过 Google DevTools MCP 列页。星相自己的 MCP 目前只管理环境。 |
| [代理出口](docs/proxy-audit.md) | 40 个本机 SOCKS5 节点中，浏览器 HTTP 和 HTTPS 出口检查各成功 38 个；HTTPS 只观察到 23 个不同出口。节点数不能当作独立 IP 数。 |
| [WebRTC、DNS 与断线](docs/proxy-audit.md) | 40 个代理环境均未采到 WebRTC 公网候选，直连对照能采到；断代理后测试请求失败，未观察到回退直连。仅确认目标域名交给 SOCKS5 代理，未验证完整 DNS 泄漏或 GeoIP。 |

## 开发

环境：Node.js 22.12+、npm、Rust 1.90.0，以及对应平台的 [Tauri 开发依赖](https://v2.tauri.app/start/prerequisites/)。仓库通过 `rust-toolchain.toml` 固定 Rust 版本。

```sh
npm install
npm run tauri dev
```

仅预览前端：

```sh
npm run dev
```

前端地址为 `http://127.0.0.1:1420`。预览数据写入浏览器 localStorage，与桌面工作空间相互独立。

## 连接本机 Camoufox

macOS 自动检测 `/Applications/Camoufox.app` 和 `~/Applications/Camoufox.app`；也可以在「配置内核」中选择应用或填写 `camoufox` 可执行文件路径。要求保留完整应用目录及其中的 `application.ini`。

1. 新建一个本地直连环境。
2. 点击「打开」，浏览器会进入星相启动页，显示实际语言、时区、屏幕尺寸和数据保留标记。
3. 关闭后再次打开，使用同一个 profile；浏览器自身退出后，列表状态会自动更新。
4. 要退出星相，请先关闭其管理的浏览器实例。星相只向自己启动的进程请求正常关闭，不操作用户原有的 Camoufox 窗口。

每个环境的数据保存在 `app_data_dir()/profiles/<环境 ID>/`，包含浏览器数据库、插件数据及 `browser.log`。运行中的环境不能修改启动参数、归档或从备份移除。复制环境只复制配置，重新创建数据目录，不复制登录状态。当前 JSON 备份不包含这些浏览器数据库。

内核路径保存在本机 `app_data_dir()/browser.json`，扩展库保存在 `extensions.json` 和 `extensions/`，都不随工作空间备份导出。扩展须有固定的 `browser_specific_settings.gecko.id` 或旧版 `applications.gecko.id`。插件自身的云同步由其账号设置决定。

## 验证和打包

```sh
npm test
npm run test:control
npm run build
cargo test --release --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

真实内核集成测试（默认跳过；使用临时环境和本机 HTTP 服务）：

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml real_camoufox -- --ignored --nocapture
```

若内核不在默认位置，运行测试前设置 `STARPERSONA_CAMOUFOX` 为应用或可执行文件路径。测试覆盖同时运行、实际基础参数、重复启动拦截、Cookie / localStorage / 共享插件数据的隔离与重启保留。

macOS 只生成 `.app`：

```sh
npm run tauri build -- --bundles app
```

产物位于 `src-tauri/target/release/bundle/macos/StarPersona.app`。当前在 Apple Silicon macOS 上验证；Windows、Linux 的平台构建和实际运行仍需分别验收。发布版本尚未签名、公证。

## 数据

桌面数据文件为 Tauri 的 `app_data_dir()/workspace.json`。macOS 对应：

```text
~/Library/Application Support/app.starpersona.desktop/workspace.json
```

配置只保存在本机，不含账号密码。写入先同步临时文件，再替换原文件。数据损坏时显示错误并停止写入，不会用空白或示例数据覆盖原配置。备份是完整工作空间；恢复会替换当前配置，请先导出备份。

快捷键：`⌘/Ctrl + K` 搜索，非输入状态下 `N` 新建环境，`Esc` 关闭弹窗。

## AI 管理：MCP / CLI / Skill

已提供 11 个 MCP 管理工具、输出 JSON 的 CLI 和可复用 Skill。先打开桌面应用，再执行 `npm run cli -- status` 或 `npm run cli -- list`。MCP 配置见 `mcp-config.example.json`；Skill 位于 `skills/starpersona/SKILL.md`。当前本机控制入口支持 macOS / Linux，Windows 管道待实现；Linux 需在浏览器窗口中正常退出。

这些入口与桌面界面共用 Rust 管理器，覆盖环境查询、创建、修改、启动、正常关闭与归档恢复。网页点击、填表等自动化尚未接入。Google Chrome DevTools MCP 基于 Chrome，不能直接控制当前 Camoufox。完整方案表、上游 issue 与接入步骤见 [AI 自动化说明](docs/automation.md)。

## 从 StarPort 增量导入代理

在桌面版 **代理 IP → 从 StarPort 导入代理列表**。StarPort 是独立的本机服务：它用自己的 mihomo 把 Clash 节点映射到本机 SOCKS5 端口。星相只读 `127.0.0.1:18765`，不启动、停止或修改 StarPort，不需要 WorkBuddyAPI，不接触订阅地址和密码。

- 导入前逐条通过代理访问固定 HTTPS 204 测试站，最多 4 个并发。服务不可用、有待应用配置、检测失败、源映射变化或端口冲突时，保留原代理。上游缺少一条也不删除，改名不覆盖自定义名称。
- 新版 StarPort 返回稳定的本机节点 UUID：同一上游配置换名或换监听端口仍能识别；共用公网出口的不同节点不会仅凭出口 IP 合并。旧版 StarPort 只能按本地协议、主机和监听端口去重，不能保证跨端口识别。已有环境引用的代理 ID 保留。
- **测连通**记录完整 HTTPS 请求的耗时，不是 ICMP ping。**测速**通过所选代理从 Cloudflare 下载 1 MiB，显示含建连的平均 Mbps；不会直连回退，也不会因测速失败删除上次成功值。单一测试站失败不代表所有网站都不可达。

**Apple Silicon macOS 本机验收（2026-11-18）：** 旧版 StarPort 返回 40 个监听端口，40 个通过本次 HTTPS 204 检测；用同一批检测结果在临时工作空间连续导入两次，分别新增 40 和 0 条。另一次重新联网检测是 39/40；在新的临时空间，1 条失败跳过、39 条新增，同一结果重放新增 0 条。将候选项全部标记失败后，已保存内容逐字节不变。单端口真实测速为 889,957 字节/秒（约 7.12 Mbps），只代表该节点当时的 1 MiB 样本。新版 StarPort 的标识算法在同一份已应用配置中读到 40 个不同标识，不代表有 40 个不同公网 IP。真实数据没有写入用户工作空间。复测：`node_modules/.bin/vite-node scripts/check-starport-import.mts`（仅 macOS/Linux，本机 StarPort 须已运行）。

## 后续能力

基于 [Camoufox](https://github.com/daijro/camoufox) 继续补充完整设备指纹生成、GeoIP / WebRTC 与代理出口匹配、代理认证。当前代理启动支持无认证 SOCKS5 / HTTP，主机为域名或 IPv4。

插件在桌面版 **扩展中心** 管理，采用「一份代码 + 每环境独立数据」。可添加本地目录、xpi/zip，或 Firefox 附加组件详情页；打开开关后所有窗口加载，也可以只指定部分窗口。Chrome 商店扩展不能在当前 Camoufox 内核运行。详见 [插件共享说明](docs/extensions.md)。

## 许可证

本项目采用 [MIT License](LICENSE)。
