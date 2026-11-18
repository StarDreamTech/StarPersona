use std::{
    path::Path,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};
static UPGRADING: AtomicBool = AtomicBool::new(false);
pub fn busy() -> bool {
    UPGRADING.load(Ordering::Acquire)
}
#[tauri::command]
pub async fn brew_upgrade(app: tauri::AppHandle) -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("仅适用于 macOS Homebrew 安装".into());
    }
    if UPGRADING.swap(true, Ordering::AcqRel) {
        return Err("更新正在运行".into());
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            UPGRADING.store(false, Ordering::Release)
        }
    }
    let _reset = Reset;
    if crate::browser::has_running(&app) {
        return Err("请先关闭星相管理的浏览器环境".into());
    }
    tauri::async_runtime::spawn_blocking(|| {
        let brew = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
            .into_iter()
            .find(|x| Path::new(x).is_file())
            .ok_or("未找到 Homebrew")?;
        let cask = "stardreamtech/tap/starpersona";
        let list = Command::new(brew)
            .args(["list", "--cask", "--versions", cask])
            .output()
            .map_err(|e| e.to_string())?;
        if !list.status.success() && !list.stderr.is_empty() {
            return Err(format!(
                "Homebrew 无法确认安装状态：{}",
                String::from_utf8_lossy(&list.stderr)
                    .trim()
                    .chars()
                    .take(800)
                    .collect::<String>()
            ));
        }
        if !list.status.success()
            || !String::from_utf8_lossy(&list.stdout).lines().any(|row| {
                row.split_whitespace()
                    .next()
                    .is_some_and(|token| token == "starpersona" || token == cask)
                    && row.split_whitespace().count() > 1
            })
        {
            return Err("当前未由组织 Homebrew Cask 安装，请先用安装命令安装。".into());
        }
        for args in [vec!["update"], vec!["upgrade", "--cask", cask]] {
            let o = Command::new(brew)
                .args(args)
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .output()
                .map_err(|e| e.to_string())?;
            if !o.status.success() {
                return Err(format!(
                    "Homebrew 未完成：{}",
                    String::from_utf8_lossy(&o.stderr)
                        .chars()
                        .rev()
                        .take(700)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                ));
            }
        }
        Ok("Homebrew 检查完成。如有新版，退出后从应用程序重新打开星相。".into())
    })
    .await
    .map_err(|e| e.to_string())?
}
