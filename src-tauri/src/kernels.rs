use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};
use tauri::{Emitter, Manager};

#[derive(Clone, Deserialize, Serialize)]
pub struct Asset {
    name: String,
    size: u64,
    sha256: String,
    url: String,
}
#[derive(Serialize)]
pub struct Choice {
    version: String,
    size: u64,
    verified: bool,
    path: Option<String>,
}
const SNAPSHOT: &str = include_str!("kernels/official-macos.json");
static CATALOG: OnceLock<Mutex<Vec<Asset>>> = OnceLock::new();
static DOWNLOAD: Mutex<()> = Mutex::new(());
fn catalog() -> &'static Mutex<Vec<Asset>> {
    CATALOG.get_or_init(|| Mutex::new(serde_json::from_str(SNAPSHOT).expect("release catalog")))
}
fn label(a: &Asset) -> Option<&str> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let suffix = if cfg!(target_arch = "aarch64") {
        "-mac.arm64.zip"
    } else {
        "-mac.x86_64.zip"
    };
    let v = a.name.strip_prefix("camoufox-")?.strip_suffix(suffix)?;
    let (tag, name) = a
        .url
        .strip_prefix("https://github.com/daijro/camoufox/releases/download/")?
        .split_once('/')?;
    let valid = |s: &str| {
        !s.is_empty()
            && s.len() < 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
            && !s.contains("..")
    };
    (valid(v) && valid(tag) && name == a.name && a.size > 0 && a.size < 3_000_000_000).then_some(v)
}
fn verified(a: &Asset) -> bool {
    a.sha256.len() == 64 && a.sha256.bytes().all(|b| b.is_ascii_hexdigit())
}
fn app_in(dir: &Path) -> Option<PathBuf> {
    std::iter::once(dir.to_owned())
        .chain(
            fs::read_dir(dir)
                .ok()?
                .filter_map(|e| e.ok().map(|x| x.path())),
        )
        .find(|p| {
            p.extension().is_some_and(|x| x == "app") && p.join("Contents/MacOS/camoufox").is_file()
        })
}
fn installed(root: &Path) -> Vec<(String, PathBuf)> {
    let mut dirs = vec![PathBuf::from("/Applications/Camoufox.app")];
    if let Some(h) = std::env::var_os("HOME") {
        let home = PathBuf::from(h);
        dirs.push(home.join("Applications/Camoufox.app"));
        let official = home.join("Library/Caches/camoufox/browsers/official");
        for e in fs::read_dir(official).into_iter().flatten().flatten() {
            dirs.push(e.path());
        }
    }
    for e in fs::read_dir(root.join("kernels"))
        .into_iter()
        .flatten()
        .flatten()
    {
        if !e.file_name().to_string_lossy().starts_with('.') {
            dirs.push(e.path())
        }
    }
    dirs.into_iter()
        .filter_map(|d| {
            let p = app_in(&d)?;
            let v = super::browser::version(&p.join("Contents/MacOS/camoufox"))?;
            Some((v, p))
        })
        .collect()
}
fn choices(root: &Path) -> Vec<Choice> {
    let local = installed(root);
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for a in catalog().lock().unwrap().iter() {
        if let Some(v) = label(a) {
            if seen.insert(v.to_string()) {
                rows.push(Choice {
                    version: v.into(),
                    size: a.size,
                    verified: verified(a),
                    path: local
                        .iter()
                        .find(|(s, _)| s == v)
                        .map(|(_, p)| p.to_string_lossy().into_owned()),
                })
            }
        }
    }
    for (v, p) in local {
        if seen.insert(v.clone()) {
            rows.push(Choice {
                version: v,
                size: 0,
                verified: false,
                path: Some(p.to_string_lossy().into_owned()),
            })
        }
    }
    rows
}
#[tauri::command]
pub async fn kernel_list(app: tauri::AppHandle, refresh: bool) -> Result<Vec<Choice>, String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        if refresh {
            let resp =
                ureq::get("https://api.github.com/repos/daijro/camoufox/releases?per_page=100")
                    .set("User-Agent", "StarPersona")
                    .timeout(std::time::Duration::from_secs(25))
                    .call()
                    .map_err(|e| format!("官方清单刷新失败，保留原列表：{e}"))?;
            let releases: Vec<serde_json::Value> =
                serde_json::from_reader(resp.into_reader().take(12_000_000))
                    .map_err(|e| e.to_string())?;
            let rows: Vec<_> = releases
                .iter()
                .filter(|r| r["draft"] == false)
                .flat_map(|r| r["assets"].as_array().into_iter().flatten())
                .filter_map(|a| {
                    Some(Asset {
                        name: a["name"].as_str()?.into(),
                        size: a["size"].as_u64()?,
                        url: a["browser_download_url"].as_str()?.into(),
                        sha256: a["digest"]
                            .as_str()
                            .unwrap_or("")
                            .trim_start_matches("sha256:")
                            .into(),
                    })
                })
                .filter(|a| label(a).is_some())
                .collect();
            if rows.is_empty() {
                return Err("清单里没有适配本机的版本".into());
            }
            *catalog().lock().unwrap() = rows;
        }
        Ok(choices(&root))
    })
    .await
    .map_err(|e| e.to_string())?
}
fn unpack(zip: &Path, out: &Path) -> Result<(), String> {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    let mut z = zip::ZipArchive::new(fs::File::open(zip).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut total = 0u64;
    if z.len() > 120_000 {
        return Err("内核包文件过多".into());
    }
    for i in 0..z.len() {
        let entry = z.by_index(i).map_err(|e| e.to_string())?;
        total = total.saturating_add(entry.size());
        if total > 6_000_000_000 {
            return Err("内核解压大小超限".into());
        }
        let name = entry.enclosed_name().ok_or("不安全的压缩包路径")?;
        if name
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("不安全的压缩包路径".into());
        }
        let dest = out.join(name);
        let mode = entry.unix_mode().unwrap_or(0);
        if !matches!(mode & 0o170000, 0 | 0o040000 | 0o100000) {
            return Err("此构建含暂不支持的链接或特殊文件".into());
        }
        if entry.is_dir() {
            fs::create_dir_all(dest).map_err(|e| e.to_string())?;
            continue;
        }
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut f = fs::File::create_new(&dest).map_err(|e| e.to_string())?;
        let n = entry.size();
        if io::copy(&mut entry.take(n + 1), &mut f).map_err(|e| e.to_string())? != n {
            return Err("解压长度不符".into());
        }
        #[cfg(unix)]
        if mode & 0o111 != 0 {
            f.set_permissions(fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?
        }
    }
    Ok(())
}
fn install(root: &Path, v: &str, progress: impl Fn(u64, &str)) -> Result<PathBuf, String> {
    let _guard = DOWNLOAD.try_lock().map_err(|_| "另一个版本正在下载")?;
    if let Some((_, p)) = installed(root).into_iter().find(|(s, _)| s == v) {
        return Ok(p);
    }
    let a = catalog()
        .lock()
        .unwrap()
        .iter()
        .find(|a| label(a) == Some(v) && verified(a))
        .cloned()
        .ok_or("没有可验证的官方安装包")?;
    let base = root.join("kernels");
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let stage = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(&base)
        .map_err(|e| e.to_string())?;
    let zip = stage.path().join("kernel.zip");
    let unpacked = stage.path().join("unpacked");
    let mut f = fs::File::create(&zip).map_err(|e| e.to_string())?;
    let mut reader = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(90))
        .build()
        .get(&a.url)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader();
    let mut h = Sha256::new();
    let (mut n, mut last) = (0u64, 0u64);
    let mut buf = vec![0u8; 131072];
    loop {
        let k = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if k == 0 {
            break;
        }
        n += k as u64;
        if n > a.size {
            return Err("下载超出官方标注大小".into());
        }
        h.update(&buf[..k]);
        f.write_all(&buf[..k]).map_err(|e| e.to_string())?;
        if n - last >= 4_000_000 {
            progress(n, "下载");
            last = n
        }
    }
    if n != a.size || format!("{:x}", h.finalize()) != a.sha256 {
        return Err("内核大小或 SHA-256 校验失败，未安装".into());
    }
    drop(f);
    progress(n, "解压");
    unpack(&zip, &unpacked)?;
    let p = app_in(&unpacked).ok_or("压缩包未包含 Camoufox.app")?;
    if super::browser::version(&p.join("Contents/MacOS/camoufox")).as_deref() != Some(v) {
        return Err("内核版本与官方文件名不一致".into());
    }
    let q = format!(
        "0081;{:x};StarPersona;{}",
        chrono::Utc::now().timestamp(),
        uuid::Uuid::new_v4()
    );
    if !Command::new("/usr/bin/xattr")
        .args(["-w", "com.apple.quarantine", &q])
        .arg(&p)
        .status()
        .map_err(|e| e.to_string())?
        .success()
    {
        return Err("无法保留系统下载安全标记".into());
    }
    let rel = p.strip_prefix(&unpacked).unwrap().to_owned();
    let target = base.join(v);
    if target.exists() {
        return Err("目标已存在，未覆盖".into());
    }
    fs::rename(unpacked, &target).map_err(|e| e.to_string())?;
    Ok(target.join(rel))
}
#[tauri::command]
pub async fn kernel_install(app: tauri::AppHandle, version: String) -> Result<String, String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        install(&root, &version, |bytes, stage| {
            let _ = app.emit(
                "kernel-progress",
                serde_json::json!({"bytes":bytes,"stage":stage}),
            );
        })
        .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_trust_boundary() {
        let mut a = serde_json::from_str::<Vec<Asset>>(SNAPSHOT)
            .unwrap()
            .remove(0);
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(label(&a), Some("159.0-beta.42"));
        }
        a.url = a.url.replace("github.com/", "github.com.evil/");
        assert!(label(&a).is_none());
        a.sha256.clear();
        assert!(!verified(&a));
    }
}
#[cfg(all(test, target_os = "macos"))]
#[test]
#[ignore = "downloads a real 313 MB official archive into a temporary directory"]
fn official_install_sample() {
    let dir = tempfile::tempdir().unwrap();
    let p = install(dir.path(), "152.0.4-beta.30", |_, _| {}).unwrap();
    assert!(p.starts_with(dir.path()));
    assert!(p.join("Contents/MacOS/camoufox").exists());
    let o = Command::new("/usr/bin/du")
        .arg("-sk")
        .arg(p)
        .output()
        .unwrap();
    eprintln!(
        "installed KiB: {}",
        String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .next()
            .unwrap()
    )
}
