# 共享插件与独立数据

星相已接入本机 Camoufox。内核设置可以选择已解压的 Firefox 插件目录；所有环境每次启动时加载这些相同目录，每个环境保存自己的浏览器数据。无需 Python 运行时。

## 如何使用

1. 准备 Firefox 版本的插件，解压到固定目录，保留 `manifest.json`。
2. 在「配置内核 → 共享插件 → 添加目录」选择它，保存。
3. 打开环境。不同环境共用插件源目录，各自保存插件设置、网站 Cookie 和本地数据库。

`manifest.json` 必须带有固定的 `browser_specific_settings.gecko.id`（兼容旧版 `applications.gecko.id`）。不同目录不能使用重复 ID；保持同一插件的 ID 不变，才能在重新加载时使用原有数据。

```text
自选固定目录/
  插件A/manifest.json             # A 的代码一份
  插件B/manifest.json             # B 的代码一份

app_data_dir()/
  browser.json                   # 本机内核路径、共享插件目录
  profiles/
    <环境ID>/                    # 每个环境自己的数据、缓存、日志
```

四个环境使用同一个插件时，原始插件代码只需保存一份；四个不同插件供四个环境使用时，原始代码保存四份。每个浏览器仍有独立运行任务、内存、编译缓存和扩展元数据，不会降为一个实例的资源消耗。

## 加载及退出

Camoufox 的 `addons` 配置调用 Firefox 的临时插件加载接口，从源目录运行。星相在每次启动时重新传入这些目录，同时复用对应环境的持久化 profile。

macOS 的「关闭」使用针对该进程的系统正常退出请求，等待浏览器保存数据后更新状态。不能用强制结束来保证数据保存；实测发现 SIGTERM 可能丢失尚未落盘的网页数据，因此 macOS 不使用它。Linux 当前需从浏览器窗口正常退出；Windows 关闭流程仍待实际验收。

共享目录应在环境关闭时维护，避免运行中改动文件。当前没有自动复制到插件版本仓库，也没有自动更新、逐环境插件选择或旧版本回收。若要替换版本，先关闭所有环境，准备新的固定目录，再修改共享插件配置。不要删除仍在使用的源目录。

移除共享目录只停止下次加载，不删除 profile 中的插件数据。归档和恢复也保留 profile；复制环境只复制配置，不复制登录数据。JSON 工作空间备份不包含浏览器数据库或本机插件路径。

## 实际验收

使用本机 `/Applications/Camoufox.app`，版本 `159.0-beta.42`，在 Apple Silicon macOS 上运行真实内核测试。测试源代码见 `src-tauri/src/browser_smoke.rs`，使用临时 profile、自建测试插件和本机 HTTP 服务，不接触用户默认 profile。

验收包括多进程同时运行，实际系统标识、语言、时区与屏幕尺寸，独立 Cookie / localStorage，插件 `storage.local` 与扩展 UUID，以及正常退出后的重启保留。测试检查 profile 中没有该插件的安装目录或 XPI 副本。

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml real_camoufox -- --ignored --nocapture
```

当前结果及数量见 `docs/browser-testing.md`。这只证明测试版本和自建插件的行为；用户实际插件的权限、功能、IndexedDB 与登录流程还需逐项验证。Camoufox 使用 Firefox 扩展，不能默认兼容只有 Chrome 版本的插件。插件自身的云端同步也可能同步设置，与本地环境隔离是两回事。

## 官方实现参考

- Camoufox 使用说明：<https://camoufox.com/python/usage/>
- 内核加载入口：<https://github.com/daijro/camoufox/blob/main/browser/additions/browser/modules/CamoufoxWindow.sys.mjs>
- Firefox 临时加载实现：<https://github.com/mozilla-firefox/firefox/blob/main/toolkit/mozapps/extensions/internal/XPIProvider.sys.mjs>

源码链接是可变分支；实际验收以本机已安装版本为准。
