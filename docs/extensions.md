# 扩展中心

星相的扩展中心参照比特浏览器的本机扩展库：扩展代码只保存一份，再决定哪些窗口加载。当前内核是 Camoufox（Firefox）。Chrome 商店扩展、员工账号同步和「固定到工具栏」没有照搬。前两项依赖比特的云端账号，后一项是 Chromium 的工具栏设置。

## 如何使用

1. 打开桌面版 **扩展中心**。
2. 在「Firefox 商店」里搜索并添加，也可以选已解压目录、`.xpi` / `.zip`，或粘贴 `addons.mozilla.org` 的扩展详情页。
3. 新扩展默认不加载。打开开关后，之后启动的所有窗口都会加载；也可以只指定部分窗口。
4. 已经打开的窗口要重新打开后才生效。

`manifest.json` 必须带有固定的 `browser_specific_settings.gecko.id`（兼容旧版 `applications.gecko.id`）。没有这个 ID 的扩展进入「不可用」，不会传给 Camoufox。同一扩展 ID 只保留一份。

以前写在内核设置里的插件目录，会在第一次打开扩展中心或启动窗口时迁入，并保持「所有窗口」加载。迁移不删除原来的目录。

```text
app_data_dir()/extensions/<扩展 ID>/   # 新添加时复制的一份代码
app_data_dir()/extensions.json         # 名称、开关和指定窗口
app_data_dir()/profiles/<环境 ID>/      # 该窗口自己的插件数据
```

四个窗口使用同一个扩展时，代码保存一份；四个不同扩展保存四份。每个浏览器仍有独立进程、内存和插件数据。删除扩展只删除星相复制的代码，不删除你选择的原目录，也不清空窗口里已有的插件数据。

## 加载及退出

Camoufox 的 `addons` 配置调用 Firefox 的临时插件加载接口，从扩展目录运行。星相按该窗口的开关和指定列表传入目录，同时复用对应环境的持久化 profile。

macOS 的「关闭」使用针对该进程的系统正常退出请求，等待浏览器保存数据后更新状态。不能用强制结束来保证数据保存；实测发现 SIGTERM 可能丢失尚未落盘的网页数据，因此 macOS 不使用它。Linux 当前需从浏览器窗口正常退出；Windows 关闭流程仍待实际验收。

扩展文件最好在相关窗口关闭时更新。扩展中心的「更新」会重新读取原来的目录，或按原来的 Firefox 附加组件页面重新下载。已经打开的窗口不会中途换扩展。

从扩展中心移除只影响下次启动，不删除 profile 中的插件数据。归档和恢复也保留 profile；复制环境只复制配置，不复制登录数据。JSON 工作空间备份不包含浏览器数据库或本机扩展文件。扩展自己的云同步仍可能把设置同步出去。

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
