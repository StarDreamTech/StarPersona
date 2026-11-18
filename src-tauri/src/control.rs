use super::{browser::BrowserManager, read_file, save_file, Profile, Workspace};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    sync::Mutex,
    time::Duration,
};
use tauri::{Emitter, Manager};

const MAX_REQUEST: u64 = 256 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    method: String,
    #[serde(default)]
    params: Value,
}

fn argument<'a>(params: &'a Value, name: &str) -> Result<&'a str, String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("缺少字符串参数 {name}"))
}

fn apply_patch(profile: &mut Profile, params: &Value) -> Result<(), String> {
    let fields = params.as_object().ok_or("参数应为 JSON 对象")?;
    let mut value = serde_json::to_value(&*profile).map_err(|e| e.to_string())?;
    for (key, val) in fields {
        match key.as_str() {
            "profileId" => {}
            "name" | "os" | "language" | "timezone" | "resolution" | "proxyId" | "groupId"
            | "tags" | "notes" | "favorite" => {
                value[key] = val.clone();
            }
            _ => return Err(format!("不支持的环境字段：{key}")),
        }
    }
    *profile = serde_json::from_value(value).map_err(|e| format!("环境参数类型错误：{e}"))?;
    Ok(())
}

// All callers use the same workspace lock and owned browser manager as the desktop UI.
fn dispatch(
    root: &Path,
    manager: &mut BrowserManager,
    request: Request,
) -> Result<(Value, bool), String> {
    let params = request.params;
    if request.method == "status" {
        return Ok((
            json!({"apiVersion":1,"transport":"local-unix-socket","pageAutomation":false,"browser":manager.status(root)?}),
            false,
        ));
    }
    let path = root.join("workspace.json");
    let mut workspace: Workspace = read_file(&path)?.ok_or("请先在星相中初始化工作空间")?;
    match request.method.as_str() {
        "profile.list" => {
            let archived = params
                .get("includeArchived")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let query = params
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let profiles = workspace
                .profiles
                .iter()
                .filter(|p| {
                    (archived || !p.archived)
                        && (query.is_empty()
                            || p.name.to_lowercase().contains(&query)
                            || p.id == query)
                })
                .collect::<Vec<_>>();
            Ok((
                json!({"profiles":profiles,"groups":workspace.groups,"proxies":workspace.proxies,"runtime":manager.status(root)?}),
                false,
            ))
        }
        "profile.start" | "profile.stop" => {
            let id = argument(&params, "profileId")?;
            if request.method == "profile.start" {
                manager.launch(root, &workspace, id, false, None)?;
            } else {
                manager.stop(id)?;
            }
            Ok((
                serde_json::to_value(manager.status(root)?).map_err(|e| e.to_string())?,
                false,
            ))
        }
        "profile.create" => {
            let id = uuid::Uuid::new_v4().to_string();
            let mut profile = Profile {
                fingerprint: id.replace('-', ""),
                id,
                serial: workspace
                    .profiles
                    .iter()
                    .map(|p| p.serial)
                    .max()
                    .unwrap_or(0)
                    + 1,
                name: argument(&params, "name")?.to_owned(),
                os: workspace.settings.default_os.clone(),
                language: "zh-CN".into(),
                timezone: workspace.settings.default_timezone.clone(),
                resolution: "1920 × 1080".into(),
                created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                group_id: String::new(),
                proxy_id: String::new(),
                tags: vec![],
                notes: String::new(),
                archived: false,
                favorite: false,
            };
            apply_patch(&mut profile, &params)?;
            let result = serde_json::to_value(&profile).map_err(|e| e.to_string())?;
            workspace.profiles.push(profile);
            manager.check_workspace(&workspace)?;
            save_file(&path, &workspace)?;
            Ok((result, true))
        }
        "profile.update" | "profile.archive" => {
            let id = argument(&params, "profileId")?;
            let profile = workspace
                .profiles
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("环境不存在")?;
            if request.method == "profile.archive" {
                profile.archived = params
                    .get("archived")
                    .and_then(Value::as_bool)
                    .ok_or("缺少布尔参数 archived")?;
            } else {
                apply_patch(profile, &params)?;
            }
            let result = serde_json::to_value(&*profile).map_err(|e| e.to_string())?;
            manager.check_workspace(&workspace)?;
            save_file(&path, &workspace)?;
            Ok((result, true))
        }
        _ => Err(format!("不支持的管理操作：{}", request.method)),
    }
}

fn respond(app: &tauri::AppHandle, root: &Path, stream: &mut UnixStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new((&*stream).take(MAX_REQUEST + 1))
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let result: Result<Value, String> = (|| {
        if line.len() as u64 > MAX_REQUEST || !line.ends_with('\n') {
            return Err("请求过大或缺少换行分隔符".into());
        }
        let request: Request =
            serde_json::from_str(&line).map_err(|e| format!("请求格式错误：{e}"))?;
        let lock = app.state::<Mutex<()>>();
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        let browsers = app.state::<Mutex<BrowserManager>>();
        let mut manager = browsers.lock().map_err(|e| e.to_string())?;
        let (value, changed) = dispatch(root, &mut manager, request)?;
        if changed {
            let _ = app.emit("workspace-changed", ());
        }
        Ok(value)
    })();
    let response = match result {
        Ok(value) => json!({"result":value}),
        Err(error) => json!({"error":error}),
    };
    writeln!(stream, "{response}").map_err(|e| e.to_string())
}

pub fn start(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let root = app.path().app_data_dir()?;
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let socket = root.join("control.sock");
    if let Ok(metadata) = fs::symlink_metadata(&socket) {
        if !metadata.file_type().is_socket() {
            return Err("控制接口路径被非 socket 文件占用".into());
        }
        match UnixStream::connect(&socket) {
            Ok(_) => return Err("星相已运行，请使用现有窗口".into()),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                ) => {}
            Err(error) => return Err(error.into()),
        }
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let _ = respond(&app, &root, &mut stream);
                }
                Err(_) => break,
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_crud_validates_and_preserves_workspace() {
        let root = tempfile::tempdir().unwrap();
        save_file(
            &root.path().join("workspace.json"),
            &crate::tests::empty_workspace(),
        )
        .unwrap();
        let mut manager = BrowserManager::default();
        let mut call = |method: &str, params| {
            dispatch(
                root.path(),
                &mut manager,
                Request {
                    method: method.into(),
                    params,
                },
            )
        };
        let (profile, changed) = call("profile.create", json!({"name":"CLI 环境"})).unwrap();
        assert!(changed);
        assert_eq!(profile["proxyId"], "");
        let id = profile["id"].as_str().unwrap();
        assert!(call("profile.update", json!({"profileId":id,"name":""})).is_err());
        assert!(call("profile.update", json!({"profileId":id,"id":"../escape"})).is_err());
        let (listed, _) = call("profile.list", json!({})).unwrap();
        assert_eq!(listed["profiles"][0]["name"], "CLI 环境");
        call(
            "profile.update",
            json!({"profileId":id,"notes":"updated","language":"en-US"}),
        )
        .unwrap();
        call("profile.archive", json!({"profileId":id,"archived":true})).unwrap();
        assert_eq!(
            call("profile.list", json!({})).unwrap().0["profiles"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        call("profile.archive", json!({"profileId":id,"archived":false})).unwrap();
        assert_eq!(
            call("profile.list", json!({})).unwrap().0["profiles"][0]["notes"],
            "updated"
        );
        assert!(call("page.evaluate", json!({})).is_err());
    }
}
