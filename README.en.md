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

Use **Configure engine** to download and verify a macOS Camoufox release, or select an existing installation. The StarPersona package does not contain the engine. On macOS, StarPersona checks `/Applications/Camoufox.app` and `~/Applications/Camoufox.app`. If your copy is elsewhere, use **配置内核** (Configure engine) to select the app. Keep its bundle intact. Create a profile using a **direct connection** (本地直连), then click **打开** (Open). The workspace starts empty: no example profiles or proxies are seeded, and sample rows left by earlier versions are removed on launch while your own data stays.

## What’s new in 0.2

**Create profile → Account & startup:** open up to eight HTTP(S) pages on launch, check for duplicate platform/account aliases, or keep two profiles with the same alias from running together. Aliases do not log in to websites. **Shared bookmarks** is opt-in: after restarting profiles, new web bookmarks sync locally; existing private bookmarks are not imported. You can edit or remove a shared bookmark centrally. Optional numbered macOS Dock icons take effect on the next launch; macOS may also show the regular icon.

On Apple Silicon, Camoufox 152.0.4-beta.30 was **313 MB to download / 687 MB on disk**, versus **1.30 GB / 2.55 GB** for 159.0-beta.42. Each installed version is shared by all profiles. Use a fresh profile when testing older versions.

Once the matching GitHub Release and organization Cask are published, install with `brew install --cask stardreamtech/tap/starpersona` and update via System settings or `brew update && brew upgrade --cask stardreamtech/tap/starpersona`. Local ad-hoc signing only; no notarization. [Distribution and test notes (Chinese)](docs/desktop-distribution.md).

## Import proxies from StarPort

In the desktop app, open **代理 IP → 从 StarPort 导入代理列表**. StarPort runs its own mihomo sidecar and exposes local SOCKS5 listeners. StarPersona only reads the local service at `127.0.0.1:18765`; it neither starts nor modifies it. WorkBuddyAPI is not required. Import validates HTTPS through each candidate (four at a time), then rechecks the source mapping before saving. Failed, missing, conflicting or unapplied entries do not replace working configurations. Existing profile references and custom names are preserved. The list can be filtered by name, host, port, region, protocol and health.

With an upgraded StarPort, stable installation-scoped IDs identify the same upstream configuration across renames and port changes. Older servers deduplicate only by local protocol, host and port. Public egress IP alone is not an identity. **测连通** measures an HTTPS 204 request. **测速** downloads 1 MiB through the selected proxy from Cloudflare and reports average Mbps including connection setup. A failure at one test site is not proof that all destinations are unreachable.

**Local check (Apple Silicon macOS, 2026-11-18):** 40 of 40 listeners passed this HTTPS 204 sample. Importing the same measured snapshot into a throwaway workspace added 40 entries the first time and zero the second. A second live sample passed 39/40, skipped the failed listener and added zero on replay. Replaying all entries as failed left the workspace byte-for-byte unchanged. A separate 1 MiB download measured 889,957 bytes/s (7.12 Mbps) on one listener. Nothing was imported into the user’s real workspace. The updated ID algorithm found 40 distinct upstream configurations, not necessarily 40 different public IPs.

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

The desktop **Extension Center** keeps one copy of each Firefox extension and separate extension data per profile. Add a local directory, an xpi/zip, or an addons.mozilla.org detail page. A switch loads it in every profile; you can also limit it to selected profiles. Chrome Web Store extensions cannot run on the current Camoufox kernel. [Extension details](docs/extensions.md).

## CLI, MCP and agent skill

Start the desktop app first. From the project root:

```sh
npm run cli -- status
npm run cli -- list
```

The CLI, eleven MCP tools and the [agent skill](skills/starpersona/SKILL.md) use the same desktop manager to query, create, edit, start, stop, archive and restore profiles, and to manage the extension library. In the [MCP configuration example](mcp-config.example.json), replace `/absolute/path/to/StarPersona` with the absolute path to your checkout. Local control uses a Unix socket on macOS and Linux; Windows named pipes are not implemented. Page clicks, typing and script execution are not exposed by the app. [Automation design and tested protocols](docs/automation.md).

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
