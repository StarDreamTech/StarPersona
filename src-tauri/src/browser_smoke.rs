use super::*;
use std::{
    io::Write,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

// Opt-in integration check: uses an installed Camoufox, temporary profiles and loopback HTTP only.
#[test]
#[ignore = "requires local Camoufox; run with --ignored --nocapture"]
fn real_camoufox_isolates_and_persists_shared_addon_data() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let (tx, rx) = mpsc::channel();
    let server = thread::spawn(move || {
        while !stopping.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(value) => value,
                Err(_) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 8192];
            let mut split = 0;
            loop {
                let size = match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                bytes.extend_from_slice(&chunk[..size]);
                if let Some(index) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    split = index + 4;
                    let headers = String::from_utf8_lossy(&bytes[..index]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|n| n.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= split + length {
                        break;
                    }
                }
                if bytes.len() > 100_000 {
                    break;
                }
            }
            if split == 0 {
                continue;
            }
            let headers = String::from_utf8_lossy(&bytes[..split]);
            let request = headers.lines().next().unwrap_or("");
            let path = request.split_whitespace().nth(1).unwrap_or("/");
            let report = path.strip_prefix("/report/");
            let body = if let Some(kind) = report {
                if let Ok(mut value) = serde_json::from_slice::<Value>(&bytes[split..]) {
                    value["acceptLanguage"] = json!(headers
                        .lines()
                        .find_map(|line| line.strip_prefix("Accept-Language: "))
                        .unwrap_or(""));
                    value["httpUserAgent"] = json!(headers
                        .lines()
                        .find_map(|line| line.strip_prefix("User-Agent: "))
                        .unwrap_or(""));
                    let _ = tx.send((kind.to_owned(), value));
                } else {
                    eprintln!("unparsed report: {}", String::from_utf8_lossy(&bytes));
                }
                String::from("ok")
            } else {
                include_str!("fingerprint-probe.html").to_owned()
            };
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let addon = root.path().join("shared-addon");
    fs::create_dir(&addon).unwrap();
    fs::write(addon.join("manifest.json"), json!({"manifest_version":2,"name":"StarPersona isolation probe","version":"1.0","browser_specific_settings":{"gecko":{"id":"isolation-probe@starpersona.local"}},"permissions":["storage","http://127.0.0.1/*"],"background":{"scripts":["background.js"]}}).to_string()).unwrap();
    fs::write(addon.join("background.js"), format!(r#"(async()=>{{try{{await fetch('{origin}/report/addon-start',{{method:'POST',body:JSON.stringify({{url:browser.runtime.getURL('')}})}});const old=await browser.storage.local.get(['token','count']);const token=old.token||crypto.randomUUID();const count=(old.count||0)+1;await browser.storage.local.set({{token,count}});await fetch('{origin}/report/addon',{{method:'POST',body:JSON.stringify({{token,count,url:browser.runtime.getURL('')}})}});}}catch(e){{await fetch('{origin}/report/addon-error',{{method:'POST',body:JSON.stringify({{error:String(e)}})}});}}}})();"#)).unwrap();
    let settings = BrowserSettings {
        executable: std::env::var("STARPERSONA_CAMOUFOX").unwrap_or_default(),
        addons: vec![addon.to_string_lossy().into_owned()],
    };
    fs::write(
        root.path().join("browser.json"),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    let mut workspace = crate::tests::empty_workspace();
    let first = profile();
    let mut second = profile();
    second.id = "test-b".into();
    second.fingerprint = "fingerprint-b".into();
    second.serial = 2;
    second.os = "macOS".into();
    second.language = "ja-JP".into();
    second.timezone = "Asia/Tokyo".into();
    second.resolution = "1440 × 900".into();
    let mut third = profile();
    third.id = "test-c".into();
    third.fingerprint = "fingerprint-c".into();
    third.serial = 3;
    third.os = "Linux".into();
    third.language = "zh-CN".into();
    third.timezone = "Asia/Shanghai".into();
    let mut fourth = profile();
    fourth.id = "test-d".into();
    fourth.fingerprint = "fingerprint-d".into();
    fourth.serial = 4;
    fourth.language = "en-GB".into();
    fourth.timezone = "Europe/London".into();
    workspace.profiles = vec![first, second, third, fourth];
    let mut manager = BrowserManager::default();
    let mut pending = Vec::new();
    let mut report = |kind: &str| -> Value {
        let until = Instant::now() + Duration::from_secs(40);
        loop {
            if let Some(index) = pending
                .iter()
                .position(|(k, _): &(String, Value)| k == kind)
            {
                return pending.remove(index).1;
            }
            let message = rx
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| {
                    panic!("browser did not report {kind}; inspect temporary browser.log")
                });
            assert_ne!(message.0, "addon-error", "plugin failed: {}", message.1);
            pending.push(message);
        }
    };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        manager
            .launch(root.path(), &workspace, "test-a", true, Some(&origin))
            .unwrap();
        let page_a = report("page");
        let addon_a = report("addon");
        println!(
            "profile A ready: {}",
            json!({"page":page_a,"addon":addon_a})
        );
        assert_eq!(page_a["platform"], "Win32");
        assert_eq!(page_a["lang"], "en-US");
        assert_eq!(page_a["tz"], "America/New_York");
        assert_eq!(page_a["width"], 1920);
        assert_eq!(addon_a["count"], 1);
        manager
            .launch(root.path(), &workspace, "test-b", true, Some(&origin))
            .unwrap();
        let page_b = report("page");
        println!("profile B page ready: {page_b}");
        let addon_b = report("addon");
        assert_eq!(page_b["platform"], "MacIntel");
        assert_eq!(page_b["lang"], "ja-JP");
        assert_eq!(page_b["tz"], "Asia/Tokyo");
        assert_eq!(page_b["width"], 1440);
        assert_ne!(page_a["token"], page_b["token"]);
        assert_eq!(page_b["previous"], Value::Null);
        assert_ne!(addon_a["token"], addon_b["token"]);
        assert_eq!(addon_b["count"], 1);
        let mut pages = vec![page_a.clone(), page_b.clone()];
        let mut addons = vec![addon_a.clone(), addon_b.clone()];
        for (id, platform, lang, timezone) in [
            ("test-c", "Linux x86_64", "zh-CN", "Asia/Shanghai"),
            ("test-d", "Win32", "en-GB", "Europe/London"),
        ] {
            manager
                .launch(root.path(), &workspace, id, true, Some(&origin))
                .unwrap();
            let page = report("page");
            let addon = report("addon");
            assert_eq!(page["platform"], platform);
            assert_eq!(page["lang"], lang);
            assert_eq!(page["tz"], timezone);
            assert_eq!(page["previous"], Value::Null);
            assert_eq!(page["cookieBefore"], "");
            assert_eq!(addon["count"], 1);
            assert!(pages.iter().all(|p| p["token"] != page["token"]));
            assert!(addons
                .iter()
                .all(|a| a["token"] != addon["token"] && a["url"] != addon["url"]));
            pages.push(page);
            addons.push(addon);
        }
        assert_eq!(manager.status(root.path()).unwrap().instances.len(), 4);
        for page in &pages {
            let fingerprint = &page["fingerprint"];
            assert_eq!(page["httpUserAgent"], page["ua"]);
            for key in [
                "userAgent",
                "platform",
                "language",
                "languages",
                "timezone",
                "hardwareConcurrency",
            ] {
                assert_eq!(
                    fingerprint["main"][key], fingerprint["iframe"][key],
                    "iframe mismatch for {key}"
                );
                assert_eq!(
                    fingerprint["main"][key], fingerprint["worker"][key],
                    "worker mismatch for {key}"
                );
            }
            assert_eq!(fingerprint["main"]["webdriver"], false);
            assert!(
                fingerprint["canvas"]["dataUrl"].is_string(),
                "canvas probe failed"
            );
            assert_eq!(
                fingerprint["canvas"]["dataUrl"],
                fingerprint["canvas"]["repeatedDataUrl"]
            );
            assert_eq!(
                fingerprint["canvas"]["dataUrl"],
                fingerprint["canvas"]["freshCanvas"]
            );
        }
        assert!(manager
            .launch(root.path(), &workspace, "test-a", true, Some(&origin))
            .is_err());
        workspace.profiles[0].timezone = "UTC".into();
        assert!(manager.check_workspace(&workspace).is_err());
        workspace.profiles[0].timezone = "America/New_York".into();
        let mut restarted = Vec::new();
        for (index, id) in ["test-a", "test-b", "test-c", "test-d"].iter().enumerate() {
            manager.stop(id).unwrap();
            let until = Instant::now() + Duration::from_secs(15);
            while manager.instances.contains_key(*id) && Instant::now() < until {
                thread::sleep(Duration::from_millis(100));
                manager.reap().unwrap();
            }
            assert!(
                !manager.instances.contains_key(*id),
                "graceful close did not complete"
            );
            manager
                .launch(root.path(), &workspace, id, true, Some(&origin))
                .unwrap();
            let page_again = report("page");
            let addon_again = report("addon");
            assert_eq!(page_again["previous"], pages[index]["token"]);
            assert_eq!(page_again["cookieBefore"], pages[index]["cookie"]);
            assert_eq!(addon_again["token"], addons[index]["token"]);
            assert_eq!(addon_again["count"], 2);
            assert_eq!(addon_again["url"], addons[index]["url"]);
            restarted.push(json!({"profileId":id,"page":page_again,"addon":addon_again}));
            assert!(!root
                .path()
                .join("profiles")
                .join(id)
                .join("extensions/isolation-probe@starpersona.local")
                .exists());
            assert!(!root
                .path()
                .join("profiles")
                .join(id)
                .join("extensions/isolation-probe@starpersona.local.xpi")
                .exists());
        }
        println!(
            "REAL_BROWSER_EVIDENCE {}",
            json!({"requestedProfiles":workspace.profiles,"pages":pages,"addons":addons,"restarted":restarted,"sharedCodeDirectoryCount":1,"simultaneousProfiles":4})
        );
    }));
    for id in manager.instances.keys().cloned().collect::<Vec<_>>() {
        let _ = manager.stop(&id);
    }
    let until = Instant::now() + Duration::from_secs(15);
    while !manager.instances.is_empty() && Instant::now() < until {
        thread::sleep(Duration::from_millis(100));
        let _ = manager.reap();
    }
    if outcome.is_err() {
        for id in ["test-a", "test-b", "test-c", "test-d"] {
            eprintln!(
                "{id}: {}",
                fs::read_to_string(root.path().join("profiles").join(id).join("browser.log"))
                    .unwrap_or_default()
            );
        }
    }
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error);
    }
    assert!(manager.instances.is_empty(), "test processes did not close");
}

#[test]
fn account_rules_survive_restore_and_reject_bad_start_urls() {
    let mut w = crate::tests::empty_workspace();
    let mut a = profile();
    a.account_label = " QA ".into();
    a.platform = "Portal".into();
    a.check_duplicate = true;
    let mut b = profile();
    b.id = "test-b".into();
    b.serial = 2;
    b.platform = "portal".into();
    b.account_label = "qa".into();
    b.archived = true;
    w.profiles = vec![a, b];
    assert!(crate::validate(&w).is_ok());
    w.profiles[1].archived = false;
    assert!(crate::validate(&w).unwrap_err().contains("同一平台"));
    w.profiles.reverse();
    assert!(crate::validate(&w).is_err());
    w.profiles.truncate(1);
    for url in [
        "javascript:alert(1)",
        "https://one:two@example.org/",
        "-P",
        "file:///a",
    ] {
        w.profiles[0].start_urls = vec![url.into()];
        assert!(crate::validate(&w).is_err());
    }
}

#[test]
#[ignore = "starts a temporary Camoufox; opt in to headed Dock observation with STARPERSONA_TEST_DOCK=1"]
fn real_configured_pages_and_account_lock() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let headed = std::env::var_os("STARPERSONA_TEST_DOCK").is_some();
    let mut w = crate::tests::empty_workspace();
    if headed {
        fs::write(root.path().join("numbered-dock"), []).unwrap();
    }
    let mut a = profile();
    a.serial = 71;
    a.platform = "Portal".into();
    a.account_label = "QA".into();
    a.exclusive_account = true;
    a.start_urls = ["/one", "/two"].map(|p| format!("{base}{p}")).to_vec();
    let mut b = profile();
    b.id = "test-b".into();
    b.serial = 72;
    b.platform = "portal".into();
    b.account_label = "qa".into();
    w.profiles = vec![a, b];
    let mut m = BrowserManager::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        m.launch(root.path(), &w, "test-a", !headed, None).unwrap();
        assert!(m
            .launch(root.path(), &w, "test-b", !headed, None)
            .unwrap_err()
            .contains("同一平台"));
        let mut seen = std::collections::HashSet::new();
        let until = Instant::now() + Duration::from_secs(30);
        while Instant::now() < until && seen.len() < 2 {
            if let Ok((mut conn, _)) = listener.accept() {
                conn.set_nonblocking(false).unwrap();
                conn.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut line = String::new();
                let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(&conn), &mut line);
                eprintln!("QA HTTP {}", line.trim_end());
                if let Some(path) = line
                    .split_whitespace()
                    .nth(1)
                    .filter(|p| ["/one", "/two"].contains(p))
                {
                    seen.insert(path.to_owned());
                }
                let _=conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<title>StarPersona QA 71</title>OK");
            } else {
                thread::sleep(Duration::from_millis(25));
            }
        }
        assert_eq!(seen.len(), 2, "only saw {seen:?}");
        eprintln!("PASS two configured startup pages and exclusive account lock");
        if headed {
            w.profiles[1].account_label = "other".into();
            m.launch(root.path(), &w, "test-b", false, None).unwrap();
            thread::sleep(Duration::from_secs(3));
            #[cfg(target_os = "macos")]
            for (id, i) in &mut m.instances {
                let tile = i.tile.as_mut().unwrap();
                assert!(tile.try_wait().unwrap().is_none(), "Dock helper exited");
                eprintln!("{id} browser={} tile={}", i.child.id(), tile.id());
            }
            thread::sleep(Duration::from_secs(
                std::env::var("STARPERSONA_DOCK_OBSERVE_SECS")
                    .ok()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(5),
            ));
        }
    }));
    for (_, i) in &mut m.instances {
        let _ = i.child.kill();
        let _ = i.child.wait();
        #[cfg(target_os = "macos")]
        if let Some(t) = i.tile.as_mut() {
            let _ = t.kill();
            let _ = t.wait();
        }
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e)
    }
}
