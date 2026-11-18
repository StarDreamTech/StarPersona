use super::read_file;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::{Emitter, Manager};
use zip::ZipArchive;

const MAX_BYTES: u64 = 200 * 1024 * 1024;
const MAX_FILES: usize = 4_000;
const MAX_EXTENSIONS: usize = 100;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub icon: String,
    pub path: String,
    pub source: String,
    pub source_url: String,
    pub origin: String,
    pub scope: String,
    pub profile_ids: Vec<String>,
    pub engine: String,
    pub available: bool,
    pub reason: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    version: u8,
    extensions: Vec<ExtensionRecord>,
}

fn app_data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn store_dir(root: &Path) -> PathBuf {
    root.join("extensions")
}

fn catalog_path(root: &Path) -> PathBuf {
    root.join("extensions.json")
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| e.to_string())
}

fn save_catalog(root: &Path, extensions: &[ExtensionRecord]) -> Result<(), String> {
    if extensions.len() > MAX_EXTENSIONS {
        return Err(format!("最多保存 {MAX_EXTENSIONS} 个扩展"));
    }
    let catalog = Catalog {
        version: 1,
        extensions: extensions.to_vec(),
    };
    atomic_write(
        &catalog_path(root),
        &serde_json::to_vec_pretty(&catalog).map_err(|e| e.to_string())?,
    )
}

fn legacy_addon_paths(root: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(root.join("browser.json")) else {
        return Vec::new();
    };
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|value| {
            value.get("addons").and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
        })
        .unwrap_or_default()
}

fn clear_legacy_addons(root: &Path) -> Result<(), String> {
    let path = root.join("browser.json");
    let Ok(bytes) = fs::read(&path) else {
        return Ok(());
    };
    let mut value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if let Some(object) = value.as_object_mut() {
        object.insert("addons".into(), json!([]));
    }
    atomic_write(
        &path,
        &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
}

fn managed_path(root: &Path, path: &Path) -> bool {
    let base = store_dir(root);
    path.starts_with(&base)
        && path != base
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

fn localized_message(dir: &Path, manifest: &Value, field: &str) -> String {
    let raw = manifest
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let Some(key) = raw
        .strip_prefix("__MSG_")
        .and_then(|value| value.strip_suffix("__"))
    else {
        return raw.to_owned();
    };
    let locale = manifest
        .get("default_locale")
        .and_then(Value::as_str)
        .unwrap_or("en");
    let path = dir.join("_locales").join(locale).join("messages.json");
    let Ok(bytes) = fs::read(path) else {
        return raw.to_owned();
    };
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|messages| {
            messages
                .get(key)
                .and_then(|entry| entry.get("message"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| raw.to_owned())
}

fn icon_data(dir: &Path, manifest: &Value) -> String {
    let icons = manifest
        .get("icons")
        .or_else(|| {
            manifest
                .get("action")
                .and_then(|action| action.get("default_icon"))
        })
        .or_else(|| {
            manifest
                .get("browser_action")
                .and_then(|action| action.get("default_icon"))
        });
    let relative = match icons {
        Some(Value::String(path)) => Some(path.clone()),
        Some(Value::Object(map)) => map
            .iter()
            .filter_map(|(size, path)| size.parse::<u32>().ok().map(|size| (size, path)))
            .max_by_key(|(size, _)| *size)
            .and_then(|(_, path)| path.as_str().map(str::to_owned)),
        _ => None,
    };
    let Some(relative) = relative else {
        return String::new();
    };
    if relative.starts_with('/') || relative.contains("..") {
        return String::new();
    }
    let path = dir.join(relative);
    let Ok(bytes) = fs::read(&path) else {
        return String::new();
    };
    if bytes.len() > 96 * 1024 {
        return String::new();
    }
    let mime = match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => return String::new(),
    };
    format!("data:{mime};base64,{}", base64_encode(&bytes))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 63) as usize] as char);
        out.push(TABLE[((triple >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn gecko_id(manifest: &Value) -> Option<String> {
    manifest
        .pointer("/browser_specific_settings/gecko/id")
        .or_else(|| manifest.pointer("/applications/gecko/id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| {
            (1..=120).contains(&id.chars().count())
                && !id.contains(['/', '\\', ' '])
                && !id.contains("..")
        })
        .map(str::to_owned)
}

fn inspect_dir(dir: &Path) -> Result<ExtensionRecord, String> {
    let manifest_path = dir.join("manifest.json");
    let metadata =
        fs::metadata(&manifest_path).map_err(|_| "目录里没有 manifest.json，请选择扩展根目录")?;
    if metadata.len() > 2 * 1024 * 1024 {
        return Err("manifest.json 过大".into());
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|_| "manifest.json 不是有效的 JSON")?;
    let name = localized_message(dir, &manifest, "name");
    let description = localized_message(dir, &manifest, "description");
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if name.is_empty() || version.is_empty() {
        return Err("扩展清单缺少名称或版本".into());
    }
    let (engine, available, reason, id) = if let Some(id) = gecko_id(&manifest) {
        ("firefox".to_owned(), true, String::new(), id)
    } else {
        (
            "chromium".to_owned(),
            false,
            "当前内核是 Camoufox（Firefox），这份扩展没有 Firefox 扩展 ID，不能加载".to_owned(),
            format!("chromium-{}", fnv(&dir.to_string_lossy())),
        )
    };
    Ok(ExtensionRecord {
        id,
        name: trim_chars(&name, 120),
        description: trim_chars(&description, 400),
        version: trim_chars(&version, 40),
        icon: icon_data(dir, &manifest),
        path: String::new(),
        source: "local".into(),
        source_url: String::new(),
        origin: String::new(),
        scope: "off".into(),
        profile_ids: Vec::new(),
        engine,
        available,
        reason,
    })
}

fn trim_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn fnv(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn copy_tree(
    source: &Path,
    destination: &Path,
    used: &mut u64,
    files: &mut usize,
) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| format!("无法读取扩展目录：{e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".DS_Store" || name == ".git" {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err("扩展目录不能包含符号链接".into());
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target, used, files)?;
        } else if kind.is_file() {
            *files += 1;
            if *files > MAX_FILES {
                return Err("扩展文件数量过多".into());
            }
            let len = entry.metadata().map_err(|e| e.to_string())?.len();
            *used += len;
            if *used > MAX_BYTES {
                return Err("扩展超过 200 MB，没有导入".into());
            }
            fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn store_name(id: &str) -> String {
    let mut name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() || name == "." || name == ".." {
        name = "extension".into();
    }
    name.chars().take(80).collect()
}

fn place_copy(root: &Path, source: &Path, id: &str) -> Result<PathBuf, String> {
    let destination = store_dir(root).join(store_name(id));
    let staging = store_dir(root).join(format!("{}.new", store_name(id)));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    let mut used = 0;
    let mut files = 0;
    if let Err(error) = copy_tree(source, &staging, &mut used, &mut files) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
    }
    fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
    destination.canonicalize().map_err(|e| e.to_string())
}

fn extract_archive(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| format!("无法打开安装包：{e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|_| "这不是有效的 zip 或 xpi 安装包")?;
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let root = destination.canonicalize().map_err(|e| e.to_string())?;
    let mut used = 0u64;
    if zip.len() > MAX_FILES {
        return Err("安装包文件数量过多".into());
    }
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if name.contains('\\')
            || Path::new(&name).is_absolute()
            || name.split(['/', '\\']).any(|part| part == "..")
        {
            return Err("安装包包含不安全的路径".into());
        }
        let target = root.join(&name);
        if !target.starts_with(&root) {
            return Err("安装包包含不安全的路径".into());
        }
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = File::create(&target).map_err(|e| e.to_string())?;
        let mut chunk = [0u8; 64 * 1024];
        loop {
            let read = entry.read(&mut chunk).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            used += read as u64;
            if used > MAX_BYTES {
                return Err("安装包解压后超过 200 MB".into());
            }
            output
                .write_all(&chunk[..read])
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn unpacked_root(dir: &Path) -> Result<PathBuf, String> {
    if dir.join("manifest.json").is_file() {
        return Ok(dir.to_path_buf());
    }
    let mut nested = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
            && entry.path().join("manifest.json").is_file()
        {
            nested.push(entry.path());
        }
    }
    if nested.len() == 1 {
        Ok(nested.remove(0))
    } else {
        Err("安装包里没有找到 manifest.json".into())
    }
}

fn load_catalog(root: &Path) -> Result<Vec<ExtensionRecord>, String> {
    let path = catalog_path(root);
    if !path.exists() {
        let mut imported: Vec<ExtensionRecord> = Vec::new();
        for addon in legacy_addon_paths(root) {
            match reference_directory(&addon) {
                Ok(mut record) => {
                    if imported.iter().any(|item| item.id == record.id) {
                        continue;
                    }
                    record.scope = "all".into();
                    imported.push(record);
                }
                Err(error) => imported.push(ExtensionRecord {
                    id: format!("missing-{}", fnv(&addon)),
                    name: Path::new(&addon)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("未命名扩展")
                        .to_owned(),
                    description: String::new(),
                    version: String::new(),
                    icon: String::new(),
                    path: addon,
                    source: "local".into(),
                    source_url: String::new(),
                    origin: String::new(),
                    scope: "off".into(),
                    profile_ids: Vec::new(),
                    engine: "firefox".into(),
                    available: false,
                    reason: error,
                }),
            }
        }
        save_catalog(root, &imported)?;
        clear_legacy_addons(root)?;
        return Ok(imported);
    }
    let catalog: Catalog = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("扩展库损坏：{e}"))?;
    if catalog.version != 1 {
        return Err("扩展库版本不受支持".into());
    }
    let mut extensions = catalog.extensions;
    let mut changed = false;
    for record in &mut extensions {
        let before = (
            record.available,
            record.reason.clone(),
            record.name.clone(),
            record.version.clone(),
        );
        refresh_record(record);
        if before
            != (
                record.available,
                record.reason.clone(),
                record.name.clone(),
                record.version.clone(),
            )
        {
            changed = true;
        }
    }
    if changed {
        save_catalog(root, &extensions)?;
    }
    Ok(extensions)
}

fn refresh_record(record: &mut ExtensionRecord) {
    let dir = PathBuf::from(&record.path);
    if !dir.join("manifest.json").is_file() {
        record.available = false;
        record.reason = "扩展目录不存在，或清单已丢失".into();
        return;
    }
    match inspect_dir(&dir) {
        Ok(inspected) => {
            record.name = inspected.name;
            record.description = if inspected.description.is_empty() {
                record.description.clone()
            } else {
                inspected.description
            };
            record.version = inspected.version;
            if record.icon.is_empty() {
                record.icon = inspected.icon;
            }
            if inspected.engine == "firefox" {
                if record.id != inspected.id && !record.id.starts_with("missing-") {
                    record.available = false;
                    record.reason =
                        "清单里的 Firefox 扩展 ID 已改变，为避免串数据，已停止加载".into();
                    return;
                }
                record.engine = "firefox".into();
                record.available = true;
                record.reason.clear();
            } else {
                record.engine = "chromium".into();
                record.available = false;
                record.reason = inspected.reason;
            }
        }
        Err(error) => {
            record.available = false;
            record.reason = error;
        }
    }
}

fn reference_directory(path: &str) -> Result<ExtensionRecord, String> {
    let dir = Path::new(path)
        .canonicalize()
        .map_err(|_| format!("插件目录不可用：{path}"))?;
    let mut record = inspect_dir(&dir)?;
    if record.engine != "firefox" {
        return Err(record.reason);
    }
    record.path = dir.to_string_lossy().into_owned();
    record.origin = record.path.clone();
    record.source = "local".into();
    Ok(record)
}

pub fn list(root: &Path) -> Result<Vec<ExtensionRecord>, String> {
    load_catalog(root)
}

pub fn paths_for(root: &Path, profile_id: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for record in load_catalog(root)? {
        let selected = match record.scope.as_str() {
            "all" => record.available && record.engine == "firefox",
            "profiles" => {
                record.available
                    && record.engine == "firefox"
                    && record.profile_ids.iter().any(|id| id == profile_id)
            }
            _ => false,
        };
        if selected && !paths.contains(&record.path) {
            paths.push(record.path);
        }
    }
    if paths.len() > 30 {
        return Err("一个窗口最多加载 30 个扩展".into());
    }
    Ok(paths)
}

fn import_unpacked(
    root: &Path,
    source: &Path,
    source_kind: &str,
    source_url: &str,
    origin: &str,
) -> Result<Vec<ExtensionRecord>, String> {
    let mut extensions = load_catalog(root)?;
    let inspected = inspect_dir(source)?;
    if extensions.iter().any(|item| item.id == inspected.id) {
        return Err(format!("扩展已存在：{}", inspected.name));
    }
    if extensions.len() >= MAX_EXTENSIONS {
        return Err(format!("最多保存 {MAX_EXTENSIONS} 个扩展"));
    }
    let stored = place_copy(root, source, &inspected.id)?;
    let mut record = inspect_dir(&stored)?;
    record.id = inspected.id;
    record.source = source_kind.into();
    record.source_url = source_url.into();
    record.origin = origin.into();
    record.path = stored.to_string_lossy().into_owned();
    extensions.push(record);
    save_catalog(root, &extensions)?;
    Ok(extensions)
}

pub fn import_path(root: &Path, input: &str) -> Result<Vec<ExtensionRecord>, String> {
    let path = PathBuf::from(input)
        .canonicalize()
        .map_err(|_| "找不到选择的文件或目录")?;
    if path.is_dir() {
        return import_unpacked(root, &path, "local", "", &path.to_string_lossy());
    }
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != "xpi" && extension != "zip" {
        return Err("请选择扩展目录，或 .xpi / .zip 安装包".into());
    }
    let temp = store_dir(root).join(format!("unpack-{}", fnv(&path.to_string_lossy())));
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| e.to_string())?;
    }
    let result = (|| {
        extract_archive(&path, &temp)?;
        let unpacked = unpacked_root(&temp)?;
        import_unpacked(root, &unpacked, "xpi", "", &path.to_string_lossy())
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

pub fn classify_extension_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    if !url.starts_with("https://") || url.chars().count() > 500 {
        return Err("请输入 https 开头的扩展地址".into());
    }
    let lower = url.to_ascii_lowercase();
    if lower.contains("chrome.google.com/webstore") || lower.contains("chromewebstore.google.com") {
        return Err("这是 Chrome 商店地址。星相当前内核是 Camoufox（Firefox），不能加载 Chrome 扩展。请改用 addons.mozilla.org 的 Firefox 附加组件。".into());
    }
    if lower.contains("addons.mozilla.org/") {
        if let Some(slug) = lower
            .split("/addon/")
            .nth(1)
            .and_then(|rest| rest.split(['/', '?', '#']).next())
        {
            if !slug.is_empty()
                && slug.len() <= 80
                && slug
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Ok(format!("amo:{slug}"));
            }
        }
        if lower.contains(".xpi") {
            return Ok(format!("xpi:{url}"));
        }
        return Err("请粘贴 Firefox 附加组件详情页，例如 https://addons.mozilla.org/firefox/addon/ublock-origin/".into());
    }
    Err("只接受 addons.mozilla.org 的扩展详情页。Chrome 商店扩展不能在当前内核运行。".into())
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    http_get_with(url, "下载扩展失败")
}

fn http_get_with(url: &str, label: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("下载地址必须是 https".into());
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(45))
        .redirects(5)
        .user_agent("StarPersona/0.1 (extension center)")
        .build();
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("{label}：{error}"))?;
    let mut reader = response.into_reader().take(MAX_BYTES + 1);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("扩展下载超过 200 MB".into());
    }
    Ok(bytes)
}

pub fn import_url(root: &Path, url: &str) -> Result<Vec<ExtensionRecord>, String> {
    let kind = classify_extension_url(url)?;
    let (download, page) = if let Some(slug) = kind.strip_prefix("amo:") {
        let api = format!("https://addons.mozilla.org/api/v5/addons/addon/{slug}/?lang=zh-CN");
        let payload: Value =
            serde_json::from_slice(&http_get(&api)?).map_err(|_| "附加组件信息无法识别")?;
        let file = payload
            .pointer("/current_version/file/url")
            .and_then(Value::as_str)
            .filter(|file| file.starts_with("https://addons.mozilla.org/"))
            .ok_or("附加组件没有可下载的安装包")?;
        (file.to_owned(), url.trim().to_owned())
    } else if let Some(file) = kind.strip_prefix("xpi:") {
        if !file
            .to_ascii_lowercase()
            .starts_with("https://addons.mozilla.org/")
        {
            return Err("只从 addons.mozilla.org 下载安装包".into());
        }
        (file.to_owned(), file.to_owned())
    } else {
        return Err("无法识别扩展地址".into());
    };
    let bytes = http_get(&download)?;
    let temp_file = store_dir(root).join(format!("download-{}.xpi", fnv(&download)));
    fs::create_dir_all(store_dir(root)).map_err(|e| e.to_string())?;
    fs::write(&temp_file, &bytes).map_err(|e| e.to_string())?;
    let temp = store_dir(root).join(format!("unpack-{}", fnv(&download)));
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| e.to_string())?;
    }
    let result = (|| {
        extract_archive(&temp_file, &temp)?;
        let unpacked = unpacked_root(&temp)?;
        import_unpacked(root, &unpacked, "amo", &page, &page)
    })();
    let _ = fs::remove_file(&temp_file);
    let _ = fs::remove_dir_all(&temp);
    result
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePage {
    pub query: String,
    pub page: u32,
    pub page_count: u32,
    pub total: u64,
    pub addons: Vec<StoreAddon>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreAddon {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub icon: String,
    pub author: String,
    pub users: u64,
    pub page_url: String,
    pub guid: String,
}

fn amo_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 80
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn form_escape(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(byte as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn localized(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_owned(),
        Value::Object(map) => {
            for key in ["zh-CN", "zh-cn", "en-US", "en-us"] {
                if let Some(text) = map.get(key).and_then(Value::as_str).map(str::trim) {
                    if !text.is_empty() {
                        return text.to_owned();
                    }
                }
            }
            map.values()
                .find_map(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_owned()
        }
        _ => String::new(),
    }
}

fn parse_store(query: &str, page: u32, bytes: &[u8]) -> Result<StorePage, String> {
    let payload: Value = serde_json::from_slice(bytes).map_err(|_| "附加组件商店暂时无法读取")?;
    let total = payload.get("count").and_then(Value::as_u64).unwrap_or(0);
    let page_count = payload
        .get("page_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(20) as u32;
    let mut addons = Vec::new();
    if let Some(results) = payload.get("results").and_then(Value::as_array) {
        for item in results {
            if item
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind != "extension")
            {
                continue;
            }
            let Some(slug) = item
                .get("slug")
                .and_then(Value::as_str)
                .filter(|slug| amo_slug(slug))
            else {
                continue;
            };
            let name = localized(item.get("name").unwrap_or(&Value::Null));
            let mut summary = localized(item.get("summary").unwrap_or(&Value::Null));
            if summary.chars().count() > 160 {
                summary = format!("{}…", summary.chars().take(160).collect::<String>());
            }
            let icon = item.get("icon_url").and_then(Value::as_str).unwrap_or("");
            addons.push(StoreAddon {
                page_url: format!("https://addons.mozilla.org/zh-CN/firefox/addon/{slug}/"),
                name: if name.is_empty() {
                    slug.to_owned()
                } else {
                    name
                },
                summary,
                version: item
                    .pointer("/current_version/version")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                icon: if icon.starts_with("https://addons.mozilla.org/") {
                    icon.to_owned()
                } else {
                    String::new()
                },
                author: item
                    .get("authors")
                    .and_then(Value::as_array)
                    .and_then(|list| list.first())
                    .and_then(|author| author.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                users: item
                    .get("average_daily_users")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                guid: item
                    .get("guid")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                slug: slug.to_owned(),
            });
            if addons.len() == 24 {
                break;
            }
        }
    }
    Ok(StorePage {
        query: query.to_owned(),
        page,
        page_count,
        total,
        addons,
    })
}

pub fn search_store(query: &str, page: u32) -> Result<StorePage, String> {
    let query = query.trim();
    if query.chars().count() > 80 {
        return Err("搜索词请控制在 80 字以内".into());
    }
    if query.chars().any(char::is_control) {
        return Err("搜索词包含无法使用的字符".into());
    }
    let page = if page == 0 { 1 } else { page.min(20) };
    let sort = if query.is_empty() { "&sort=users" } else { "" };
    let url = format!(
        "https://addons.mozilla.org/api/v5/addons/search/?app=firefox&type=extension&lang=zh-CN&page_size=24&page={page}{sort}&q={}",
        form_escape(query)
    );
    parse_store(query, page, &http_get_with(&url, "搜索附加组件失败")?)
}

fn known_profile_ids(root: &Path) -> Result<Vec<String>, String> {
    let workspace = read_file(&root.join("workspace.json"))?.ok_or("请先创建工作空间")?;
    Ok(workspace
        .profiles
        .into_iter()
        .map(|profile| profile.id)
        .collect())
}

pub fn set_scope(
    root: &Path,
    id: &str,
    scope: &str,
    profile_ids: &[String],
) -> Result<Vec<ExtensionRecord>, String> {
    if !matches!(scope, "all" | "profiles" | "off") {
        return Err("加载范围只能是所有窗口、指定窗口或不加载".into());
    }
    let mut extensions = load_catalog(root)?;
    let record = extensions
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or("扩展不存在")?;
    if !record.available {
        return Err(format!("这个扩展当前不可用：{}", record.reason));
    }
    if scope == "profiles" {
        if profile_ids.is_empty() {
            return Err("请至少选择一个窗口".into());
        }
        let known = known_profile_ids(root)?;
        if profile_ids
            .iter()
            .any(|profile_id| !known.iter().any(|id| id == profile_id))
        {
            return Err("选择的窗口不存在".into());
        }
        record.profile_ids = profile_ids.to_vec();
    }
    record.scope = scope.into();
    save_catalog(root, &extensions)?;
    Ok(extensions)
}

pub fn remove(root: &Path, id: &str) -> Result<Vec<ExtensionRecord>, String> {
    let mut extensions = load_catalog(root)?;
    let index = extensions
        .iter()
        .position(|item| item.id == id)
        .ok_or("扩展不存在")?;
    let record = extensions.remove(index);
    let path = PathBuf::from(&record.path);
    if managed_path(root, &path) {
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| format!("扩展文件未能删除：{e}"))?;
        }
    }
    save_catalog(root, &extensions)?;
    Ok(extensions)
}

pub fn update(root: &Path, id: &str) -> Result<Vec<ExtensionRecord>, String> {
    let extensions = load_catalog(root)?;
    let record = extensions
        .iter()
        .find(|item| item.id == id)
        .ok_or("扩展不存在")?
        .clone();
    if record.source == "amo" && record.source_url.starts_with("https://") {
        let without = extensions
            .into_iter()
            .filter(|item| item.id != id)
            .collect::<Vec<_>>();
        save_catalog(root, &without)?;
        match import_url(root, &record.source_url) {
            Ok(mut imported) => {
                if let Some(updated) = imported.iter_mut().find(|item| item.id == record.id) {
                    updated.scope = record.scope;
                    updated.profile_ids = record.profile_ids;
                    save_catalog(root, &imported)?;
                }
                return Ok(load_catalog(root)?);
            }
            Err(error) => {
                let mut restored = load_catalog(root)?;
                if !restored.iter().any(|item| item.id == record.id) {
                    restored.push(record);
                    save_catalog(root, &restored)?;
                }
                return Err(error);
            }
        }
    }
    let origin = PathBuf::from(&record.origin);
    if origin.is_dir() && origin.join("manifest.json").is_file() {
        let stored = if managed_path(root, Path::new(&record.path)) {
            place_copy(root, &origin, &record.id)?
        } else {
            origin.canonicalize().map_err(|e| e.to_string())?
        };
        let mut extensions = load_catalog(root)?;
        let current = extensions
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or("扩展不存在")?;
        let inspected = inspect_dir(&stored)?;
        if inspected.engine == "firefox" && inspected.id != current.id {
            return Err("更新后的 Firefox 扩展 ID 变了，已保留原版本".into());
        }
        current.name = inspected.name;
        current.description = inspected.description;
        current.version = inspected.version;
        current.icon = inspected.icon;
        current.path = stored.to_string_lossy().into_owned();
        current.engine = inspected.engine;
        current.available = inspected.available;
        current.reason = inspected.reason;
        save_catalog(root, &extensions)?;
        return Ok(extensions);
    }
    Err("找不到原来的目录或下载地址，无法更新".into())
}

fn publish(
    app: &tauri::AppHandle,
    extensions: Vec<ExtensionRecord>,
) -> Result<Vec<ExtensionRecord>, String> {
    let _ = app.emit("extensions-changed", ());
    Ok(extensions)
}

#[tauri::command]
pub fn extension_list(
    app: tauri::AppHandle,
    lock: tauri::State<'_, std::sync::Mutex<()>>,
) -> Result<Vec<ExtensionRecord>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    load_catalog(&app_data(&app)?)
}

#[tauri::command]
pub fn extension_import_path(
    app: tauri::AppHandle,
    lock: tauri::State<'_, std::sync::Mutex<()>>,
    path: String,
) -> Result<Vec<ExtensionRecord>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    let root = app_data(&app)?;
    publish(&app, import_path(&root, &path)?)
}

#[tauri::command]
pub fn extension_import_url(
    app: tauri::AppHandle,
    lock: tauri::State<'_, std::sync::Mutex<()>>,
    url: String,
) -> Result<Vec<ExtensionRecord>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    publish(&app, import_url(&app_data(&app)?, &url)?)
}

#[tauri::command]
pub fn extension_set_scope(
    app: tauri::AppHandle,
    lock: tauri::State<'_, std::sync::Mutex<()>>,
    id: String,
    scope: String,
    profile_ids: Vec<String>,
) -> Result<Vec<ExtensionRecord>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    publish(
        &app,
        set_scope(&app_data(&app)?, &id, &scope, &profile_ids)?,
    )
}

#[tauri::command]
pub fn extension_remove(
    app: tauri::AppHandle,
    lock: tauri::State<'_, std::sync::Mutex<()>>,
    id: String,
) -> Result<Vec<ExtensionRecord>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    publish(&app, remove(&app_data(&app)?, &id)?)
}

#[tauri::command]
pub fn extension_update(
    app: tauri::AppHandle,
    lock: tauri::State<'_, std::sync::Mutex<()>>,
    id: String,
) -> Result<Vec<ExtensionRecord>, String> {
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    publish(&app, update(&app_data(&app)?, &id)?)
}

#[tauri::command]
pub fn extension_store_search(query: String, page: u32) -> Result<StorePage, String> {
    search_store(&query, page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn manifest(id: &str) -> String {
        format!(
            r#"{{"manifest_version":2,"name":"隔离探针","version":"1.2.0","description":"测试","browser_specific_settings":{{"gecko":{{"id":"{id}"}}}}}}"#
        )
    }

    fn write_extension(dir: &Path, id: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("manifest.json"), manifest(id)).unwrap();
    }

    #[test]
    fn imports_one_shared_copy_and_assigns_scope() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        write_extension(source.path(), "probe@starpersona.local");
        let listed = import_path(root.path(), &source.path().to_string_lossy()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].scope, "off");
        assert!(listed[0].available);
        assert_ne!(listed[0].path, source.path().to_string_lossy());
        assert!(Path::new(&listed[0].path).join("manifest.json").is_file());
        assert!(import_path(root.path(), &source.path().to_string_lossy())
            .unwrap_err()
            .contains("已存在"));
        assert!(paths_for(root.path(), "env-a").unwrap().is_empty());
        fs::write(root.path().join("workspace.json"), r#"{"version":1,"profiles":[{"id":"env-a","serial":1,"name":"甲","groupId":"","proxyId":"","os":"macOS","tags":[],"notes":"","createdAt":"2026-11-18T00:00:00Z","archived":false,"favorite":false,"language":"zh-CN","timezone":"Asia/Shanghai","resolution":"1440 × 900","fingerprint":"abc"}],"groups":[],"proxies":[],"settings":{"name":"t","defaultOs":"macOS","defaultTimezone":"Asia/Shanghai"}}"#).unwrap();
        set_scope(
            root.path(),
            "probe@starpersona.local",
            "profiles",
            &["env-a".into()],
        )
        .unwrap();
        assert_eq!(paths_for(root.path(), "env-a").unwrap().len(), 1);
        assert!(paths_for(root.path(), "env-b").unwrap().is_empty());
        set_scope(root.path(), "probe@starpersona.local", "all", &[]).unwrap();
        assert_eq!(paths_for(root.path(), "env-b").unwrap().len(), 1);
        let origin = source.path().join("manifest.json");
        remove(root.path(), "probe@starpersona.local").unwrap();
        assert!(origin.is_file());
        assert!(paths_for(root.path(), "env-a").unwrap().is_empty());
    }

    #[test]
    fn keeps_user_directory_when_removing_a_referenced_extension() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        write_extension(source.path(), "keep@starpersona.local");
        fs::write(
            root.path().join("browser.json"),
            serde_json::to_vec(&json!({"executable":"","addons":[source.path()]})).unwrap(),
        )
        .unwrap();
        assert_eq!(paths_for(root.path(), "any").unwrap().len(), 1);
        assert!(legacy_addon_paths(root.path()).is_empty());
        remove(root.path(), "keep@starpersona.local").unwrap();
        assert!(source.path().join("manifest.json").is_file());
    }

    #[test]
    fn chrome_extension_is_unavailable_and_chrome_store_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.json"),
            r#"{"manifest_version":3,"name":"Chrome Only","version":"1.0.0"}"#,
        )
        .unwrap();
        let listed = import_path(root.path(), &source.path().to_string_lossy()).unwrap();
        assert!(!listed[0].available);
        assert_eq!(listed[0].engine, "chromium");
        assert!(paths_for(root.path(), "env-a").unwrap().is_empty());
        let error = classify_extension_url(
            "https://chromewebstore.google.com/detail/metamask/nkbihfbeogaeaoehlefnkodbefgpgknn",
        )
        .unwrap_err();
        assert!(error.contains("Camoufox"));
        assert!(classify_extension_url(
            "https://addons.mozilla.org/zh-CN/firefox/addon/ublock-origin/"
        )
        .unwrap()
        .starts_with("amo:"));
    }

    #[test]
    fn store_results_keep_only_firefox_addons_and_our_page_url() {
        let page = parse_store(
            "拦截",
            1,
            br#"{"count":2,"page_count":80,"results":[
                {"type":"extension","slug":"ublock-origin","guid":"uBlock0@raymondhill.net","name":{"zh-CN":"uBlock Origin"},"summary":{"en-US":"blocker"},"icon_url":"https://addons.mozilla.org/user-media/a.png","url":"https://evil.example/addon/ublock-origin/","authors":[{"name":"Raymond Hill"}],"average_daily_users":10,"current_version":{"version":"1.76.4"}},
                {"type":"extension","slug":"../bad","name":"no"},
                {"type":"statictheme","slug":"a-theme","name":"Theme"},
                {"type":"extension","slug":"remote-icon","name":"Remote","icon_url":"https://evil.example/icon.png"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(page.page_count, 20);
        assert_eq!(page.addons.len(), 2);
        assert_eq!(page.addons[0].name, "uBlock Origin");
        assert_eq!(
            page.addons[0].page_url,
            "https://addons.mozilla.org/zh-CN/firefox/addon/ublock-origin/"
        );
        assert_eq!(page.addons[0].author, "Raymond Hill");
        assert!(page.addons[0]
            .icon
            .starts_with("https://addons.mozilla.org/"));
        assert!(page.addons[1].icon.is_empty());
        assert!(search_store("x".repeat(81).as_str(), 1).is_err());
    }

    #[test]
    fn rejects_symlink_and_zip_escape() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        write_extension(source.path(), "link@starpersona.local");
        symlink(
            source.path().join("manifest.json"),
            source.path().join("alias.json"),
        )
        .unwrap();
        assert!(import_path(root.path(), &source.path().to_string_lossy()).is_err());
        let archive_path = root.path().join("bad.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../escape.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"no").unwrap();
        writer.finish().unwrap();
        assert!(import_path(root.path(), &archive_path.to_string_lossy())
            .unwrap_err()
            .contains("不安全"));
        assert!(!root.path().join("escape.txt").exists());
    }

    #[test]
    fn resolves_localized_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("_locales/zh-CN")).unwrap();
        fs::write(dir.path().join("manifest.json"), r#"{"manifest_version":2,"name":"__MSG_extName__","description":"__MSG_extDesc__","default_locale":"zh-CN","version":"3","browser_specific_settings":{"gecko":{"id":"i18n@starpersona.local"}}}"#).unwrap();
        fs::write(
            dir.path().join("_locales/zh-CN/messages.json"),
            r#"{"extName":{"message":"中文名"},"extDesc":{"message":"中文说明"}}"#,
        )
        .unwrap();
        let inspected = inspect_dir(dir.path()).unwrap();
        assert_eq!(inspected.name, "中文名");
        assert_eq!(inspected.description, "中文说明");
    }
}
