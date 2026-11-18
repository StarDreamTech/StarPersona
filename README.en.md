# StarPersona (星相)

<img src="public/icon.svg" width="64" height="64" alt="StarPersona icon">

[简体中文](README.md) · **English**

StarPersona is a local desktop app for managing [Camoufox](https://github.com/daijro/camoufox) browser profiles. Each profile has its own browser data directory. The app uses Tauri 2, Rust, Vue 3 and TypeScript. It has been tested on Apple Silicon macOS; Windows and Linux builds and runtime behavior have not yet been verified.

## Open the app on a Mac

If you have already built the macOS app, run this from the project root:

```sh
open src-tauri/target/release/bundle/macos/StarPersona.app
```

A fresh clone contains source code. To launch the desktop app from source, install Node.js 22.12 or later, npm, the pinned Rust 1.90.0 toolchain and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). Then run:

```sh
npm install
npm run tauri dev
```

Install Camoufox separately to launch browser profiles. On macOS, StarPersona checks `/Applications/Camoufox.app` and `~/Applications/Camoufox.app`. If your copy is elsewhere, use **配置内核** (Configure engine) to select the app. Keep its bundle intact. Create a profile using a **direct connection** (本地直连), then click **打开** (Open). On first launch you will see eight example profiles and three example proxies. The sample proxy addresses are reserved for documentation and cannot be used to launch a browser.

## What was tested

These results are from **November 18, 2026**, on Apple Silicon macOS with **Camoufox 159.0-beta.42**. The linked reports contain the methods and detailed results in Chinese.

| Check | Observed result |
| --- | --- |
| [Profiles and extensions](docs/browser-testing.md) | Four simultaneous profiles shared one unpacked test extension. Cookies, localStorage and extension data stayed separate and survived restart. No profile contained an installed copy of the extension. |
| [Device fingerprints](docs/protocol-and-fingerprint-audit.md) | OS identifiers, language, time zone and screen dimensions matched the settings. All four Canvas results changed after restart; all three profiles configured for Windows or Linux still reported an Apple GPU. **The profiles did not pass the stable device fingerprint check.** |
| [Page control](docs/protocol-and-fingerprint-audit.md) | Camoufox returned HTTP 404 on CDP discovery. A WebDriver BiDi session could list pages and evaluate a script. Google’s Chrome DevTools MCP listed pages on the Chrome 157 control. StarPersona’s own MCP currently manages profiles only. |
| [Proxy exits](docs/proxy-audit.md) | Of 40 local SOCKS5 nodes, 38 returned an exit address in each of the browser HTTP and HTTPS checks. Only 23 distinct HTTPS exit addresses were observed. Several nodes shared an exit or changed exits between samples. |
| [WebRTC, DNS and outages](docs/proxy-audit.md) | No public WebRTC candidate was observed in the 40 proxied profiles; the direct control did produce one. Test requests failed when the proxy went down, with no observed fallback to a direct connection. The browser passed target hostnames to SOCKS5; full DNS leak testing and GeoIP matching remain unverified. |

## Manage profiles and extensions

Create, edit, duplicate, search, group, favorite or archive profiles. Configure language, time zone, resolution and an optional unauthenticated SOCKS5 or HTTP proxy. The manager tracks the processes it starts. Close managed browsers before quitting StarPersona. On Linux, close the browser from its own window.

Each profile has a persistent data directory. Duplicating a profile copies its settings, not its signed-in sessions. JSON backups cover workspace configuration only: browser databases, extension code and local engine paths are excluded. On macOS the workspace is stored at `~/Library/Application Support/app.starpersona.desktop/workspace.json`.

For shared extensions, select an unpacked Firefox extension directory with a stable Gecko ID in its manifest. The configured set applies to all profiles. The code is shared; each profile keeps its own extension storage. An extension’s own cloud sync may still synchronize its settings. [Extension details](docs/extensions.md).

## CLI, MCP and agent skill

Start the desktop app first. From the project root:

```sh
npm run cli -- status
npm run cli -- list
```

The CLI, seven MCP tools and the [agent skill](skills/starpersona/SKILL.md) use the same desktop manager to query, create, edit, start, stop, archive and restore profiles. In the [MCP configuration example](mcp-config.example.json), replace `/absolute/path/to/StarPersona` with the absolute path to your checkout. Local control uses a Unix socket on macOS and Linux; Windows named pipes are not implemented. Page clicks, typing and script execution are not exposed by the app. [Automation design and tested protocols](docs/automation.md).

## Build and test

```sh
npm test
npm run test:control
npm run build
cargo test --release --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --bundles app
```

The last command produces the macOS `.app` at the path above. It is not signed for distribution or notarized. `npm run dev` starts only the frontend preview at `http://127.0.0.1:1420`; its localStorage is separate from the desktop workspace.

The real-browser integration tests are skipped by default. To repeat the four-profile isolation and extension check with your installed Camoufox:

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml real_camoufox -- --ignored --nocapture
```

If Camoufox is outside the default locations, set `STARPERSONA_CAMOUFOX` to its app or executable path. The [proxy report](docs/proxy-audit.md) describes the separate opt-in run using a real proxy pool.

## License

[MIT](LICENSE).
