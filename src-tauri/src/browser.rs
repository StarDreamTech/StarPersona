use super::{read_file, Profile, Proxy, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
};
use tauri::{Manager, State};

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserSettings {
    executable: String,
    addons: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    settings: BrowserSettings,
    path: Option<String>,
    version: Option<String>,
    instances: Vec<InstanceStatus>,
    errors: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceStatus {
    profile_id: String,
    pid: u32,
    stopping: bool,
}

struct Instance {
    child: Child,
    signature: String,
    stopping: bool,
    log: PathBuf,
    #[cfg(target_os = "macos")]
    tile: Option<Child>,
}

#[derive(Default)]
pub struct BrowserManager {
    instances: BTreeMap<String, Instance>,
    errors: HashMap<String, String>,
}

fn read_settings(root: &Path) -> Result<BrowserSettings, String> {
    match fs::read(root.join("browser.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("内核设置损坏：{e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BrowserSettings::default()),
        Err(e) => Err(e.to_string()),
    }
}

fn resolve_executable(input: &str) -> Option<PathBuf> {
    let mut candidates = if input.trim().is_empty() {
        vec![
            PathBuf::from("/Applications/Camoufox.app"),
            PathBuf::from("/usr/bin/camoufox"),
            PathBuf::from("/usr/local/bin/camoufox"),
        ]
    } else {
        vec![PathBuf::from(input)]
    };
    if input.trim().is_empty() {
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join("Applications/Camoufox.app"));
        }
        for variable in ["PROGRAMFILES", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(variable) {
                candidates.push(PathBuf::from(base).join("Camoufox/camoufox.exe"));
            }
        }
    }
    candidates.into_iter().find_map(|path| {
        let path = if path.extension().is_some_and(|e| e == "app") {
            path.join("Contents/MacOS/camoufox")
        } else {
            path
        };
        let path = path.canonicalize().ok()?;
        (path.is_file() && matches!(path.file_name()?.to_str()?, "camoufox" | "camoufox.exe"))
            .then_some(path)
    })
}

fn resources(executable: &Path) -> PathBuf {
    let parent = executable.parent().unwrap_or(Path::new("."));
    if parent.file_name().is_some_and(|n| n == "MacOS") {
        parent.parent().unwrap_or(parent).join("Resources")
    } else {
        parent.to_path_buf()
    }
}

pub(super) fn version(executable: &Path) -> Option<String> {
    fs::read_to_string(resources(executable).join("application.ini"))
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("Version=").map(str::to_owned))
}

fn addon_paths(settings: &BrowserSettings) -> Result<Vec<String>, String> {
    if settings.addons.len() > 30 {
        return Err("最多加载 30 个共享插件".into());
    }
    let mut paths = Vec::new();
    let mut ids = HashSet::new();
    for path in &settings.addons {
        let dir = Path::new(path)
            .canonicalize()
            .map_err(|e| format!("插件目录不可用：{e}"))?;
        let path = dir.to_string_lossy().into_owned();
        if paths.contains(&path) {
            continue;
        }
        let manifest_path = dir.join("manifest.json");
        let metadata = fs::metadata(&manifest_path)
            .map_err(|_| "请选择包含 manifest.json 的已解压 Firefox 插件目录")?;
        if metadata.len() > 2 * 1024 * 1024 {
            return Err("插件 manifest.json 过大".into());
        }
        let manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let id = manifest
            .pointer("/browser_specific_settings/gecko/id")
            .or_else(|| manifest.pointer("/applications/gecko/id"))
            .and_then(Value::as_str);
        let id = id
            .filter(|id| !id.trim().is_empty())
            .ok_or("插件需要固定的 Firefox 扩展 ID，以便重启后保留数据")?;
        if !ids.insert(id.to_owned()) {
            return Err(format!(
                "多个插件目录使用相同扩展 ID：{id}，请只保留一个版本"
            ));
        }
        paths.push(path);
    }
    Ok(paths)
}

fn signature(workspace: &Workspace, profile: &Profile) -> Result<String, String> {
    serde_json::to_string(&(
        &profile.os,
        &profile.language,
        &profile.timezone,
        &profile.resolution,
        &profile.fingerprint,
        profile.account_key(),
        profile.exclusive_account,
        &profile.start_urls,
        workspace
            .proxies
            .iter()
            .find(|p| p.id == profile.proxy_id)
            .map(|p| (&p.id, &p.protocol, &p.host, p.port)),
    ))
    .map_err(|e| e.to_string())
}

fn profile_directory(root: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.len() > 100
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("环境 ID 只能包含字母、数字、横线和下划线".into());
    }
    let base = root.join("profiles");
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let dir = base.join(id);
    if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("环境目录不能是符号链接".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if !dir
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&base)
    {
        return Err("环境目录越界".into());
    }
    Ok(dir)
}

fn launch_config(profile: &Profile, major: u32, addons: Vec<String>) -> Result<Value, String> {
    let (platform, oscpu, ua_os) = match profile.os.as_str() {
        "Windows" => (
            "Win32",
            "Windows NT 10.0; Win64; x64",
            "Windows NT 10.0; Win64; x64",
        ),
        "macOS" => (
            "MacIntel",
            "Intel Mac OS X 10.15",
            "Macintosh; Intel Mac OS X 10.15",
        ),
        "Linux" => ("Linux x86_64", "Linux x86_64", "X11; Linux x86_64"),
        _ => return Err("不支持的操作系统配置".into()),
    };
    if !["zh-CN", "en-US", "en-GB", "ja-JP"].contains(&profile.language.as_str()) {
        return Err("请选择界面支持的浏览器语言".into());
    }
    if ![
        "Asia/Shanghai",
        "America/Los_Angeles",
        "America/New_York",
        "Asia/Tokyo",
        "Asia/Singapore",
        "Europe/London",
        "UTC",
    ]
    .contains(&profile.timezone.as_str())
    {
        return Err("请选择界面支持的时区".into());
    }
    let (width, height) = match profile.resolution.as_str() {
        "1920 × 1080" => (1920, 1080),
        "1440 × 900" => (1440, 900),
        "2560 × 1440" => (2560, 1440),
        "1366 × 768" => (1366, 768),
        _ => return Err("请选择界面支持的屏幕尺寸".into()),
    };
    let ua = format!("Mozilla/5.0 ({ua_os}; rv:{major}.0) Gecko/20100101 Firefox/{major}.0");
    let mut config = json!({
        "navigator.userAgent": ua, "headers.User-Agent": ua,
        "navigator.appVersion": format!("5.0 ({ua_os})"),
        "navigator.platform": platform, "navigator.oscpu": oscpu,
        "navigator.language": profile.language, "headers.Accept-Language": format!("{},{};q=0.9", profile.language, profile.language.split('-').next().unwrap_or("en")),
        "locale:all": profile.language, "timezone": profile.timezone,
        "screen.width": width, "screen.height": height,
        "screen.availWidth": width, "screen.availHeight": height - 40,
        "addons": addons
    });
    // shortcut: only the exposed settings are injected; full coherent device fingerprints need a generator.
    config["locale:language"] = json!(profile.language.split('-').next().unwrap_or("en"));
    config["locale:region"] = json!(profile.language.split('-').nth(1).unwrap_or("US"));
    Ok(config)
}

fn proxy_prefs(proxy: Option<&Proxy>) -> Result<Value, String> {
    let mut prefs = json!({
        "network.proxy.type": 0, "network.proxy.no_proxies_on": "localhost, 127.0.0.1, ::1",
        "network.proxy.failover_direct": false, "network.proxy.socks": "", "network.proxy.socks_port": 0,
        "network.proxy.socks_version": 5, "network.proxy.socks_remote_dns": false,
        "network.proxy.http": "", "network.proxy.http_port": 0, "network.proxy.ssl": "", "network.proxy.ssl_port": 0
    });
    let Some(proxy) = proxy else {
        return Ok(prefs);
    };
    let host = proxy.host.trim();
    if ["192.0.2.", "198.51.100.", "203.0.113."]
        .iter()
        .any(|prefix| host.starts_with(prefix))
    {
        return Err("这是文档示例代理，无法联网。请编辑为真实代理，或将环境改为本地直连。".into());
    }
    if host.is_empty()
        || host.contains(['/', ':', '@'])
        || host.chars().any(char::is_whitespace)
        || proxy.port == 0
    {
        return Err("代理主机应为域名或 IPv4，不含协议、端口或账号".into());
    }
    prefs["network.proxy.type"] = json!(1);
    match proxy.protocol.as_str() {
        "SOCKS5" => {
            prefs["network.proxy.socks"] = json!(host);
            prefs["network.proxy.socks_port"] = json!(proxy.port);
            prefs["network.proxy.socks_version"] = json!(5);
            prefs["network.proxy.socks_remote_dns"] = json!(true);
        }
        "HTTP" => {
            prefs["network.proxy.http"] = json!(host);
            prefs["network.proxy.http_port"] = json!(proxy.port);
            prefs["network.proxy.ssl"] = json!(host);
            prefs["network.proxy.ssl_port"] = json!(proxy.port);
        }
        _ => return Err("代理协议不受支持".into()),
    }
    Ok(prefs)
}

fn log_tail(path: &Path) -> String {
    let Ok(mut file) = fs::File::open(path) else {
        return String::new();
    };
    let _ = file
        .seek(SeekFrom::End(-2048))
        .or_else(|_| file.seek(SeekFrom::Start(0)));
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    text
}

impl BrowserManager {
    fn reap(&mut self) -> Result<(), String> {
        let mut ended = Vec::new();
        for (id, instance) in &mut self.instances {
            #[cfg(target_os = "macos")]
            if let Some(t) = instance.tile.as_mut() {
                if let Some(status) = t.try_wait().map_err(|e| e.to_string())? {
                    if !status.success() && !instance.stopping {
                        self.errors.insert(id.clone(),"Dock 编号不可用。浏览器仍在运行；可在内核设置关闭编号，重开恢复原生图标。".into());
                    }
                    instance.tile = None;
                }
            }
            if let Some(status) = instance.child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() && !instance.stopping {
                    self.errors.insert(
                        id.clone(),
                        format!("浏览器已退出（{status}）。{}", log_tail(&instance.log)),
                    );
                }
                ended.push(id.clone());
            }
        }
        for id in ended {
            #[cfg(target_os = "macos")]
            if let Some(tile) = self.instances.get_mut(&id).and_then(|i| i.tile.take()) {
                std::thread::spawn(move || {
                    let mut tile = tile;
                    let _ = tile.wait();
                });
            }
            self.instances.remove(&id);
        }
        Ok(())
    }

    pub fn check_workspace(&mut self, workspace: &Workspace) -> Result<(), String> {
        self.reap()?;
        for (id, instance) in &self.instances {
            let profile = workspace
                .profiles
                .iter()
                .find(|p| &p.id == id && !p.archived)
                .ok_or("请先关闭运行中的环境，再归档或替换配置")?;
            if signature(workspace, profile)? != instance.signature {
                return Err("请先关闭环境，再修改它的启动配置".into());
            }
        }
        Ok(())
    }

    pub(super) fn status(&mut self, root: &Path) -> Result<BrowserStatus, String> {
        self.reap()?;
        let settings = read_settings(root)?;
        let executable = resolve_executable(&settings.executable);
        Ok(BrowserStatus {
            version: executable.as_ref().and_then(|p| version(p)),
            path: executable.map(|p| p.to_string_lossy().into_owned()),
            settings,
            instances: self
                .instances
                .iter()
                .map(|(id, i)| InstanceStatus {
                    profile_id: id.clone(),
                    pid: i.child.id(),
                    stopping: i.stopping,
                })
                .collect(),
            errors: self.errors.clone(),
        })
    }

    pub(super) fn launch(
        &mut self,
        root: &Path,
        workspace: &Workspace,
        id: &str,
        headless: bool,
        start_url: Option<&str>,
    ) -> Result<(), String> {
        self.reap()?;
        if crate::distribution::busy() {
            return Err("星相正在更新，请稍后再启动环境".into());
        }
        if self.instances.contains_key(id) {
            return Err("该环境已经运行，请勿重复打开".into());
        }
        let profile = workspace
            .profiles
            .iter()
            .find(|p| p.id == id && !p.archived)
            .ok_or("找不到可启动的环境")?;
        profile.validate_options()?;
        if let Some(key) = profile.account_key() {
            if workspace.profiles.iter().any(|p| {
                self.instances.contains_key(&p.id)
                    && (p.exclusive_account || profile.exclusive_account)
                    && p.account_key().as_ref() == Some(&key)
            }) {
                return Err("同一平台和账号已有环境运行，请先关闭它".into());
            }
        }
        let settings = read_settings(root)?;
        let executable = resolve_executable(&settings.executable)
            .ok_or("未找到 Camoufox，请在内核设置中选择已下载的应用")?;
        #[cfg(target_os = "macos")]
        if executable.starts_with(root.join("kernels")) {
            let app = executable
                .ancestors()
                .find(|p| p.extension().is_some_and(|x| x == "app"))
                .ok_or("无效的内核应用")?;
            let status = Command::new("/usr/sbin/spctl")
                .args(["--assess", "--type", "execute"])
                .arg(app)
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("macOS 未批准此下载内核，未启动。请选择系统信任的版本。".into());
            }
        }
        let version = version(&executable)
            .ok_or("无法读取 Camoufox 的 application.ini，请选择完整的内核应用")?;
        let major: u32 = version
            .split('.')
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or("浏览器版本无效")?;
        let mut addons = crate::extensions::paths_for(root, &profile.id)?;
        if let Some(addon) = crate::bookmarks::addon(root)? {
            addons.push(addon)
        }
        let dir = profile_directory(root, id)?;
        let mut config = launch_config(profile, major, addons)?;
        if headless {
            config["debug"] = json!(true);
        }
        let mut prefs = proxy_prefs(workspace.proxies.iter().find(|p| p.id == profile.proxy_id))?;
        prefs["intl.accept_languages"] = json!(format!(
            "{},{}",
            profile.language,
            profile.language.split('-').next().unwrap_or("en")
        ));
        prefs["intl.locale.requested"] = json!(profile.language);
        prefs["browser.shell.checkDefaultBrowser"] = json!(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let user_js = prefs
            .as_object()
            .ok_or("浏览器设置无效")?
            .iter()
            .map(|(key, value)| format!("user_pref({}, {});\n", json!(key), value))
            .collect::<String>();
        fs::write(dir.join("user.js"), user_js).map_err(|e| e.to_string())?;
        let page = dir.join("starpersona-start.html");
        let metadata = serde_json::to_string(&json!({"name": profile.name, "id": profile.id}))
            .map_err(|e| e.to_string())?
            .replace('<', "\\u003c");
        fs::write(
            &page,
            include_str!("start.html").replace("__PROFILE_JSON__", &metadata),
        )
        .map_err(|e| e.to_string())?;
        let urls = match start_url {
            Some(url) => vec![url.to_owned()],
            None if !profile.start_urls.is_empty() => profile.start_urls.clone(),
            None => vec![tauri::Url::from_file_path(&page)
                .map_err(|_| "启动页路径无效")?
                .to_string()],
        };
        let log_path = dir.join("browser.log");
        let log = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&log_path)
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        let helper = if !headless && root.join("numbered-dock").is_file() {
            Some(dock_helper(root)?)
        } else {
            None
        };
        let mut command = Command::new(&executable);
        #[cfg(target_os = "macos")]
        if helper.is_some() {
            command.env("MOZ_APP_NO_DOCK", "1");
        }

        command
            .args(["-no-remote", "-new-instance", "-profile"])
            .arg(&dir);
        if headless {
            command.arg("--headless");
        }
        // Do not inherit another launcher's config or debugger endpoint.
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("CAMOU_") || name.starts_with("MOZ_REMOTE_") {
                command.env_remove(key);
            }
        }
        command
            .env("STARPERSONA_BOOKMARK_SOCKET", root.join("control.sock"))
            .env("CAMOU_CONFIG_1", config.to_string())
            .env("CAMOU_PREFS_1", prefs.to_string())
            .args(urls)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(log));
        let child = command
            .spawn()
            .map_err(|e| format!("无法启动 Camoufox：{e}"))?;
        self.errors.remove(id);
        #[cfg(target_os = "macos")]
        let tile = helper.and_then(|h| {
            match Command::new(h)
                .arg(child.id().to_string())
                .arg(profile.serial.to_string())
                .arg(&executable)
                .stdin(Stdio::null())
                .spawn()
            {
                Ok(t) => Some(t),
                Err(e) => {
                    self.errors.insert(
                        id.to_string(),
                        format!("Dock 图标未启动，浏览器仍在运行：{e}"),
                    );
                    None
                }
            }
        });
        self.instances.insert(
            id.to_owned(),
            Instance {
                #[cfg(target_os = "macos")]
                tile,
                child,
                signature: signature(workspace, profile)?,
                stopping: false,
                log: log_path,
            },
        );
        Ok(())
    }

    pub(super) fn stop(&mut self, id: &str) -> Result<(), String> {
        self.reap()?;
        let instance = self.instances.get_mut(id).ok_or("环境已经关闭")?;
        if instance.stopping {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            // A normal app quit flushes site storage; SIGTERM can lose pending writes.
            let app = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(
                instance.child.id() as i32,
            )
            .ok_or("浏览器尚未就绪，请稍后重试关闭")?;
            if !app.terminate() {
                return Err("无法请求浏览器退出，请从浏览器窗口正常退出".into());
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            return Err("请从浏览器窗口正常退出，星相会自动更新状态".into());
        }
        #[cfg(windows)]
        {
            let result = Command::new("taskkill")
                .args(["/PID", &instance.child.id().to_string()])
                .status()
                .map_err(|e| e.to_string())?;
            if !result.success() {
                return Err("关闭请求失败，请从浏览器窗口关闭环境".into());
            }
        }
        instance.stopping = true;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn dock_helper(root: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let bits = include_bytes!(concat!(env!("OUT_DIR"), "/StarPersonaDock"));
    let d = root.join("helpers");
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join("StarPersonaDock");
    if fs::read(&path).ok().as_deref() != Some(bits.as_slice()) {
        let mut tmp = tempfile::NamedTempFile::new_in(d).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut tmp, bits).map_err(|e| e.to_string())?;
        tmp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        tmp.persist(&path).map_err(|e| e.to_string())?;
    }
    Ok(path)
}
#[tauri::command]
pub fn numbered_dock(app: tauri::AppHandle, enabled: Option<bool>) -> Result<bool, String> {
    let path = app_root(&app)?.join("numbered-dock");
    match enabled {
        Some(true) => fs::write(&path, []).map_err(|e| e.to_string())?,
        Some(false) if path.exists() => fs::remove_file(&path).map_err(|e| e.to_string())?,
        _ => {}
    }
    Ok(cfg!(target_os = "macos") && path.is_file())
}
fn app_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_status(
    app: tauri::AppHandle,
    browsers: State<'_, Mutex<BrowserManager>>,
) -> Result<BrowserStatus, String> {
    browsers
        .lock()
        .map_err(|e| e.to_string())?
        .status(&app_root(&app)?)
}

#[tauri::command]
pub fn save_browser_settings(
    app: tauri::AppHandle,
    browsers: State<'_, Mutex<BrowserManager>>,
    mut settings: BrowserSettings,
) -> Result<BrowserStatus, String> {
    let mut manager = browsers.lock().map_err(|e| e.to_string())?;
    manager.reap()?;
    if !manager.instances.is_empty() {
        return Err("请先关闭运行中的浏览器，再修改内核和共享插件".into());
    }
    if resolve_executable(&settings.executable).is_none() {
        return Err("请选择 Camoufox.app 或 camoufox 可执行文件".into());
    }
    settings.addons = addon_paths(&settings)?;
    let root = app_root(&app)?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let temp = root.join("browser.tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, root.join("browser.json")).map_err(|e| e.to_string())?;
    manager.status(&root)
}

#[tauri::command]
pub fn launch_browser(
    app: tauri::AppHandle,
    lock: State<'_, Mutex<()>>,
    browsers: State<'_, Mutex<BrowserManager>>,
    profile_id: String,
) -> Result<BrowserStatus, String> {
    let _lock = lock.lock().map_err(|e| e.to_string())?;
    let root = app_root(&app)?;
    let workspace = read_file(&root.join("workspace.json"))?.ok_or("请先保存工作空间")?;
    let mut manager = browsers.lock().map_err(|e| e.to_string())?;
    manager.launch(&root, &workspace, &profile_id, false, None)?;
    manager.status(&root)
}

#[tauri::command]
pub fn stop_browser(
    app: tauri::AppHandle,
    browsers: State<'_, Mutex<BrowserManager>>,
    profile_id: String,
) -> Result<BrowserStatus, String> {
    let mut manager = browsers.lock().map_err(|e| e.to_string())?;
    manager.stop(&profile_id)?;
    manager.status(&app_root(&app)?)
}

pub fn has_running(app: &tauri::AppHandle) -> bool {
    let manager = app.state::<Mutex<BrowserManager>>();
    let running = match manager.lock() {
        Ok(mut m) => m.reap().is_err() || !m.instances.is_empty(),
        Err(_) => true,
    };
    running
}

#[cfg(test)]
mod tests {
    use super::*;
    mod smoke {
        include!("browser_smoke.rs");
    }
    mod proxy_smoke {
        include!("proxy_smoke.rs");
    }

    fn profile() -> Profile {
        serde_json::from_value(json!({"id":"test-a","serial":1,"name":"测试","groupId":"","proxyId":"","os":"Windows","tags":[],"notes":"","createdAt":"2026-11-18T00:00:00Z","archived":false,"favorite":false,"language":"en-US","timezone":"America/New_York","resolution":"1920 × 1080","fingerprint":"test"})).unwrap()
    }

    #[test]
    fn measurements_do_not_change_the_running_proxy_signature() {
        let mut w = crate::tests::empty_workspace();
        w.proxies.push(serde_json::from_value(json!({"id":"p","name":"local","protocol":"SOCKS5","host":"127.0.0.1","port":18001,"country":""})).unwrap());
        let mut p = profile();
        p.proxy_id = "p".into();
        let before = signature(&w, &p).unwrap();
        w.proxies[0].check = Some(
            serde_json::from_value(json!({"at":"now","ok":true,"message":"","lastGoodMs":23}))
                .unwrap(),
        );
        assert_eq!(before, signature(&w, &p).unwrap());
        w.proxies[0].port += 1;
        assert_ne!(before, signature(&w, &p).unwrap());
    }

    #[test]
    fn rejects_profile_path_escape() {
        let root = tempfile::tempdir().unwrap();
        for id in ["../outside", "/tmp/outside", "a/b", ".."] {
            assert!(profile_directory(root.path(), id).is_err());
        }
        assert!(profile_directory(root.path(), "test-a")
            .unwrap()
            .starts_with(root.path().canonicalize().unwrap()));
    }

    #[test]
    fn injects_matching_language_ua_and_dimensions() {
        let config = launch_config(&profile(), 159, vec![]).unwrap();
        assert_eq!(config["navigator.platform"], "Win32");
        assert_eq!(config["navigator.language"], "en-US");
        assert_eq!(config["timezone"], "America/New_York");
        assert_eq!(config["headers.User-Agent"], config["navigator.userAgent"]);
        assert_eq!(config["screen.width"], 1920);
    }

    #[test]
    fn proxy_switch_clears_old_mode_and_rejects_examples() {
        let mut p: Proxy = serde_json::from_value(json!({"id":"p","name":"proxy","protocol":"SOCKS5","host":"127.0.0.1","port":1080,"country":""})).unwrap();
        assert_eq!(
            proxy_prefs(Some(&p)).unwrap()["network.proxy.socks_remote_dns"],
            true
        );
        assert_eq!(proxy_prefs(None).unwrap()["network.proxy.type"], 0);
        p.host = "192.0.2.10".into();
        assert!(proxy_prefs(Some(&p)).is_err());
    }

    #[test]
    fn shared_addons_require_stable_id() {
        let dir = tempfile::tempdir().unwrap();
        let settings = BrowserSettings {
            executable: String::new(),
            addons: vec![dir.path().to_string_lossy().into_owned()],
        };
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"manifest_version":2,"name":"test","version":"1"}"#,
        )
        .unwrap();
        assert!(addon_paths(&settings).is_err());
        fs::write(dir.path().join("manifest.json"), r#"{"manifest_version":2,"name":"test","version":"1","browser_specific_settings":{"gecko":{"id":"test@starpersona.local"}}}"#).unwrap();
        assert_eq!(addon_paths(&settings).unwrap().len(), 1);
        let other = tempfile::tempdir().unwrap();
        fs::copy(
            dir.path().join("manifest.json"),
            other.path().join("manifest.json"),
        )
        .unwrap();
        let mut duplicates = settings.clone();
        duplicates.addons.push(settings.addons[0].clone());
        assert_eq!(addon_paths(&duplicates).unwrap().len(), 1);
        duplicates
            .addons
            .push(other.path().to_string_lossy().into_owned());
        assert!(addon_paths(&duplicates)
            .unwrap_err()
            .contains("相同扩展 ID"));
    }
}
