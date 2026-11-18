use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path, sync::Mutex};
use tauri::Manager;
#[derive(Serialize, Deserialize, Clone, Default)]
struct Library {
    enabled: bool,
    #[serde(default)]
    epoch: String,
    items: std::collections::BTreeMap<String, Entry>,
}
#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Entry {
    title: String,
    url: String,
}
#[derive(Deserialize)]
struct Change {
    id: String,
    before: Option<Entry>,
    after: Option<Entry>,
}
fn read(root: &Path) -> Result<Library, String> {
    match fs::read(root.join("bookmarks.json")) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("书签文件无法读取：{e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Library::default()),
        Err(e) => Err(e.to_string()),
    }
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or("无效路径")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut f = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|_| f.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    f.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub(super) fn apply(root: &Path, kind: &str, params: Value) -> Result<Value, String> {
    let mut lib = read(root)?;
    let mut dirty = false;
    let mut conflicts = 0;
    match kind {
        "list" => {}
        "enable" => {
            let enabled = params["enabled"].as_bool().ok_or("缺少同步开关")?;
            if enabled && (!lib.enabled || lib.epoch.is_empty()) {
                lib.epoch = uuid::Uuid::new_v4().to_string();
            }
            lib.enabled = enabled;
            dirty = true
        }
        "edit" | "sync" => {
            if kind == "sync" && !lib.enabled {
                return Ok(json!({"enabled":false}));
            }
            if kind == "sync" && params["epoch"].as_str() != Some(&lib.epoch) {
                return Ok(json!({"enabled":true,"reset":true,"epoch":lib.epoch}));
            }
            let changes: Vec<Change> =
                serde_json::from_value(params["changes"].clone()).map_err(|_| "无效的书签更改")?;
            if changes.len() > 500 {
                return Err("单次最多更新 500 条书签".into());
            }
            for c in changes {
                uuid::Uuid::parse_str(&c.id).map_err(|_| "书签 ID 无效")?;
                if lib.items.get(&c.id) != c.before.as_ref() {
                    conflicts += 1;
                    continue;
                }
                if let Some(e) = c.after {
                    let url = tauri::Url::parse(&e.url).map_err(|_| "网址无效")?;
                    if !matches!(url.scheme(), "http" | "https")
                        || !url.username().is_empty()
                        || url.password().is_some()
                        || e.url.len() > 4096
                        || e.title.chars().count() > 500
                    {
                        return Err("仅支持无内嵌密码的 HTTP(S) 书签，标题最多 500 字".into());
                    }
                    lib.items.insert(c.id, e);
                } else {
                    lib.items.remove(&c.id);
                }
                dirty = true;
            }
        }
        _ => return Err("未知书签操作".into()),
    }
    let mut result = serde_json::to_value(&lib).map_err(|e| e.to_string())?;
    if dirty {
        let bytes = serde_json::to_vec(&lib).map_err(|e| e.to_string())?;
        if bytes.len() > 700_000 || lib.items.len() > 2500 {
            return Err("共享书签库超出容量（2500 条或 700 KB）".into());
        }
        atomic(&root.join("bookmarks.json"), &bytes)?
    }
    result["conflicts"] = json!(conflicts);
    Ok(result)
}
#[tauri::command]
pub fn bookmark_action(
    app: tauri::AppHandle,
    lock: tauri::State<'_, Mutex<()>>,
    kind: String,
    params: Value,
) -> Result<Value, String> {
    let _g = lock.lock().map_err(|e| e.to_string())?;
    apply(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        &kind,
        params,
    )
}
pub(super) fn addon(root: &Path) -> Result<Option<String>, String> {
    if !read(root)?.enabled {
        return Ok(None);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err("本平台暂不支持浏览器书签同步".into());
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let home = std::env::var_os("HOME").ok_or("无用户目录")?;
        let dir = Path::new(&home).join(if cfg!(target_os = "macos") {
            "Library/Application Support/Mozilla/NativeMessagingHosts"
        } else {
            ".mozilla/native-messaging-hosts"
        });
        let manifest = dir.join("app.starpersona.bookmarks.json");
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let host = json!({"name":"app.starpersona.bookmarks","description":"StarPersona shared bookmarks","path":exe,"type":"stdio","allowed_extensions":["bookmarks@starpersona.local"]});
        let data = host.to_string();
        if fs::read(&manifest).ok().as_deref() != Some(data.as_bytes()) {
            atomic(&manifest, data.as_bytes())?
        }
        let dir = root.join("builtin-addons/bookmarks");
        for (name, contents) in [
            ("manifest.json", include_str!("bookmarks/manifest.json")),
            ("sync.js", include_str!("bookmarks/sync.js")),
        ] {
            let p = dir.join(name);
            if fs::read(&p).ok().as_deref() != Some(contents.as_bytes()) {
                atomic(&p, contents.as_bytes())?
            }
        }
        Ok(Some(dir.to_string_lossy().into_owned()))
    }
}
#[cfg(unix)]
pub fn native_host() -> bool {
    use std::io::{BufRead, BufReader, Read};
    use std::os::unix::net::UnixStream;
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|a| Path::new(a).file_name())
        != Some(std::ffi::OsStr::new("app.starpersona.bookmarks.json"))
        || args.get(2).and_then(|a| a.to_str()) != Some("bookmarks@starpersona.local")
    {
        return false;
    }
    let Some(socket) = std::env::var_os("STARPERSONA_BOOKMARK_SOCKET") else {
        return true;
    };
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut hdr = [0; 4];
        if input.read_exact(&mut hdr).is_err() {
            break;
        }
        let n = u32::from_ne_bytes(hdr) as usize;
        if n > 220_000 {
            break;
        }
        let mut msg = vec![0; n];
        if input.read_exact(&mut msg).is_err() {
            break;
        }
        let reply = (|| -> Option<Value> {
            let params: Value = serde_json::from_slice(&msg).ok()?;
            let mut s = UnixStream::connect(&socket).ok()?;
            s.set_read_timeout(Some(std::time::Duration::from_secs(8)))
                .ok()?;
            writeln!(s, "{}", json!({"method":"bookmark.sync","params":params})).ok()?;
            let mut b = Vec::new();
            BufReader::new(s.take(1_000_000))
                .read_until(b'\n', &mut b)
                .ok()?;
            serde_json::from_slice(&b).ok()
        })()
        .unwrap_or_else(|| json!({"error":"星相未连接"}));
        let b = reply.to_string();
        if output
            .write_all(&(b.len() as u32).to_ne_bytes())
            .and_then(|_| output.write_all(b.as_bytes()))
            .and_then(|_| output.flush())
            .is_err()
        {
            break;
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_by_default_and_no_stale_resurrection() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let id = uuid::Uuid::new_v4();
        let b = json!({"title":"A","url":"https://example.org/"});
        let op = |from: Value, to: Value| json!({"epoch":read(p).unwrap().epoch,"changes":[{"id":id,"before":from,"after":to}]});
        assert_eq!(
            apply(p, "sync", op(Value::Null, b.clone())).unwrap(),
            json!({"enabled":false})
        );
        apply(p, "enable", json!({"enabled":true})).unwrap();
        apply(p, "sync", op(Value::Null, b.clone())).unwrap();
        apply(p, "edit", op(b.clone(), Value::Null)).unwrap();
        assert_eq!(apply(p, "sync", op(b.clone(), b)).unwrap()["conflicts"], 1);
        assert_eq!(read(p).unwrap().items.len(), 0);
        let old = op(
            Value::Null,
            json!({"title":"disabled period","url":"https://example.org/private"}),
        );
        apply(p, "enable", json!({"enabled":false})).unwrap();
        apply(p, "enable", json!({"enabled":true})).unwrap();
        assert_eq!(apply(p, "sync", old).unwrap()["reset"], true);
        assert!(read(p).unwrap().items.is_empty());
        assert!(apply(
            p,
            "edit",
            op(Value::Null, json!({"title":"bad","url":"javascript:1"}))
        )
        .is_err())
    }
}
#[cfg(all(test, target_os = "macos"))]
#[test]
#[ignore = "real Camoufox + built StarPersona native host, two isolated profiles"]
fn two_real_windows() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    let tmp = tempfile::tempdir_in("/tmp").unwrap();
    let root = tmp.path().to_path_buf();
    let socket = root.join("s");
    let l = UnixListener::bind(&socket).unwrap();
    apply(&root, "enable", json!({"enabled":true})).unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut b = String::new();
            BufReader::new(&s).read_line(&mut b).unwrap();
            let r: Value = serde_json::from_str(&b).unwrap();
            assert_eq!(r["method"], "bookmark.sync");
            let reply = apply(&root, "sync", r["params"].clone()).unwrap();
            let _ = writeln!(s, "{}", json!({"result":reply}));
        }
    });
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/test-bookmark-windows.py");
    assert!(std::process::Command::new("python3")
        .arg(script)
        .arg(tmp.path())
        .status()
        .unwrap()
        .success());
}
