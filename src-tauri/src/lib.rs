use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path, sync::Mutex};
use tauri::{Manager, State};
mod browser;
#[cfg(unix)]
mod control;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Workspace {
    version: u8,
    profiles: Vec<Profile>,
    groups: Vec<Group>,
    proxies: Vec<Proxy>,
    settings: Settings,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    serial: u64,
    name: String,
    group_id: String,
    proxy_id: String,
    os: String,
    tags: Vec<String>,
    notes: String,
    created_at: String,
    archived: bool,
    favorite: bool,
    language: String,
    timezone: String,
    resolution: String,
    fingerprint: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Group {
    id: String,
    name: String,
    color: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Proxy {
    id: String,
    name: String,
    protocol: String,
    host: String,
    port: u16,
    country: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    name: String,
    default_os: String,
    default_timezone: String,
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}
fn valid_os(os: &str) -> bool {
    matches!(os, "Windows" | "macOS" | "Linux")
}
fn unique_ids<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    ids.into_iter()
        .all(|id| valid_text(id, 100) && seen.insert(id))
}

fn validate(w: &Workspace) -> Result<(), String> {
    if w.version != 1 || w.profiles.len() > 5000 || w.groups.len() > 500 || w.proxies.len() > 5000 {
        return Err("工作空间版本或数量超出限制".into());
    }
    if !valid_text(&w.settings.name, 80)
        || !valid_os(&w.settings.default_os)
        || !valid_text(&w.settings.default_timezone, 100)
    {
        return Err("工作空间设置无效".into());
    }
    if !unique_ids(w.profiles.iter().map(|p| p.id.as_str()))
        || !unique_ids(w.groups.iter().map(|g| g.id.as_str()))
        || !unique_ids(w.proxies.iter().map(|p| p.id.as_str()))
    {
        return Err("存在无效或重复的 ID".into());
    }
    for g in &w.groups {
        if !valid_text(&g.name, 80)
            || g.color.len() != 7
            || !g.color.starts_with('#')
            || !g.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("分组配置无效".into());
        }
    }
    for p in &w.proxies {
        if !valid_text(&p.name, 80)
            || !valid_text(&p.host, 253)
            || p.host.chars().any(char::is_whitespace)
            || p.port == 0
            || !matches!(p.protocol.as_str(), "SOCKS5" | "HTTP")
            || p.country.chars().count() > 8
        {
            return Err("代理配置无效".into());
        }
    }
    let mut serials = HashSet::new();
    for p in &w.profiles {
        if !valid_text(&p.name, 80)
            || !valid_os(&p.os)
            || p.serial == 0
            || p.serial > 9_007_199_254_740_991
            || !serials.insert(p.serial)
            || p.tags.len() > 12
            || p.tags.iter().any(|t| t.chars().count() > 30)
            || p.notes.chars().count() > 1000
            || !valid_text(&p.created_at, 100)
            || [&p.language, &p.timezone, &p.resolution, &p.fingerprint]
                .iter()
                .any(|s| !valid_text(s, 100))
        {
            return Err("环境配置无效".into());
        }
        if (!p.group_id.is_empty() && !w.groups.iter().any(|g| g.id == p.group_id))
            || (!p.proxy_id.is_empty() && !w.proxies.iter().any(|proxy| proxy.id == p.proxy_id))
        {
            return Err("环境引用的分组或代理不存在".into());
        }
    }
    Ok(())
}

fn read_file(path: &Path) -> Result<Option<Workspace>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let workspace: Workspace =
                serde_json::from_slice(&bytes).map_err(|e| format!("数据文件格式错误：{e}"))?;
            validate(&workspace)?;
            Ok(Some(workspace))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("读取失败：{e}")),
    }
}

fn save_file(path: &Path, workspace: &Workspace) -> Result<(), String> {
    validate(workspace)?;
    let parent = path.parent().ok_or("存储路径无效")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(workspace).map_err(|e| e.to_string())?;
    // Write and sync before replacing the last valid workspace.
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temp, path).map_err(|e| format!("替换工作空间失败：{e}"))
}

#[tauri::command]
fn read_workspace(
    app: tauri::AppHandle,
    lock: State<'_, Mutex<()>>,
) -> Result<Option<Workspace>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    read_file(
        &app.path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("workspace.json"),
    )
}

#[tauri::command]
fn save_workspace(
    app: tauri::AppHandle,
    lock: State<'_, Mutex<()>>,
    browsers: State<'_, Mutex<browser::BrowserManager>>,
    workspace: Workspace,
    expected: Option<Workspace>,
) -> Result<(), String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("workspace.json");
    if let Some(current) = read_file(&path)? {
        if expected.as_ref().and_then(|w| serde_json::to_value(w).ok())
            != serde_json::to_value(&current).ok()
        {
            return Err("工作空间已被其他操作更新，请刷新后重试".into());
        }
    }
    browsers
        .lock()
        .map_err(|e| e.to_string())?
        .check_workspace(&workspace)?;
    save_file(&path, &workspace)
}

pub fn run() {
    use tauri::Emitter;
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(Mutex::new(()))
        .manage(Mutex::new(browser::BrowserManager::default()))
        .setup(|app| {
            #[cfg(unix)]
            control::start(app.handle().clone())?;
            Ok(())
        })
        .menu(|app| {
            let menu = tauri::menu::Menu::default(app)?;
            #[cfg(target_os = "macos")]
            for item in menu.items()? {
                if let Some(submenu) = item.as_submenu() {
                    for (index, child) in submenu.items()?.iter().enumerate() {
                        if child
                            .as_predefined_menuitem()
                            .is_some_and(|p| p.text().is_ok_and(|s| s.starts_with("Quit ")))
                        {
                            submenu.remove(child)?;
                            submenu.insert(
                                &tauri::menu::MenuItem::with_id(
                                    app,
                                    "starpersona-quit",
                                    "退出星相",
                                    true,
                                    Some("CmdOrCtrl+Q"),
                                )?,
                                index,
                            )?;
                        }
                    }
                }
            }
            Ok(menu)
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "starpersona-quit" {
                if browser::has_running(app) {
                    let _ = app.emit("browser-close-blocked", ());
                } else {
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            read_workspace,
            save_workspace,
            browser::browser_status,
            browser::save_browser_settings,
            browser::launch_browser,
            browser::stop_browser
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if browser::has_running(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.emit("browser-close-blocked", ());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("启动 StarPersona 失败")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if browser::has_running(app) {
                    api.prevent_exit();
                    let _ = app.emit("browser-close-blocked", ());
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn empty_workspace() -> Workspace {
        Workspace {
            version: 1,
            profiles: vec![],
            groups: vec![],
            proxies: vec![],
            settings: Settings {
                name: "测试空间".into(),
                default_os: "Windows".into(),
                default_timezone: "Asia/Shanghai".into(),
            },
        }
    }

    #[test]
    fn persists_and_reloads_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        assert!(read_file(&path).unwrap().is_none());
        save_file(&path, &empty_workspace()).unwrap();
        assert_eq!(read_file(&path).unwrap().unwrap().settings.name, "测试空间");
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn invalid_save_keeps_original_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut workspace = empty_workspace();
        save_file(&path, &workspace).unwrap();
        workspace.settings.name.clear();
        assert!(save_file(&path, &workspace).is_err());
        assert_eq!(read_file(&path).unwrap().unwrap().settings.name, "测试空间");
    }

    #[test]
    fn corrupt_data_is_not_silently_reset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        fs::write(&path, "invalid").unwrap();
        assert!(read_file(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid");
    }
}
