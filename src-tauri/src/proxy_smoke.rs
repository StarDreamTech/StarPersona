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

// Opt-in collection, not a certification: uses real outbound proxies and STUN.
#[test]
#[ignore = "requires Camoufox and STARPERSONA_PROXY_POOL; makes real outbound requests"]
fn real_proxy_pool_collects_browser_evidence() {
    let path = std::env::var("STARPERSONA_PROXY_POOL").expect("set STARPERSONA_PROXY_POOL");
    let pool: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let nodes = pool["proxies"].as_array().expect("proxy pool array");
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
                Ok(v) => v,
                Err(_) => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut data = Vec::new();
            let mut chunk = [0; 8192];
            let mut split = 0;
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => data.extend_from_slice(&chunk[..n]),
                };
                if let Some(i) = data.windows(4).position(|b| b == b"\r\n\r\n") {
                    split = i + 4;
                    let length = String::from_utf8_lossy(&data[..i])
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|n| n.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= split + length {
                        break;
                    }
                }
                if data.len() > 65536 {
                    break;
                }
            }
            if split == 0 {
                continue;
            }
            let headers = String::from_utf8_lossy(&data[..split]);
            let path = headers
                .lines()
                .next()
                .unwrap_or("")
                .split_whitespace()
                .nth(1)
                .unwrap_or("/");
            let body = if path.starts_with("/report/") {
                if let Ok(value) = serde_json::from_slice::<Value>(&data[split..]) {
                    let _ = tx.send(value);
                }
                "ok".to_owned()
            } else {
                let label = path.trim_start_matches('/');
                if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                    continue;
                }
                include_str!("proxy-probe.html").replace("__LABEL__", label)
            };
            let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
        }
    });
    let mut workspace = crate::tests::empty_workspace();
    let mut labels = vec!["DIRECT".to_owned()];
    for (i, node) in nodes.iter().enumerate() {
        let label = format!("P{:02}", i + 1);
        let url = tauri::Url::parse(node["url"].as_str().unwrap()).unwrap();
        assert!(
            matches!(url.scheme(), "socks5" | "socks5h")
                && url.username().is_empty()
                && url.password().is_none(),
            "QA currently accepts unauthenticated SOCKS only"
        );
        workspace.proxies.push(serde_json::from_value(json!({"id":label,"name":label,"protocol":"SOCKS5","host":url.host_str().unwrap(),"port":url.port().unwrap(),"country":""})).unwrap());
        labels.push(label);
    }
    let unused = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = unused.local_addr().unwrap().port();
    drop(unused);
    workspace.proxies.push(serde_json::from_value(json!({"id":"DEAD","name":"DEAD","protocol":"SOCKS5","host":"127.0.0.1","port":dead_port,"country":""})).unwrap());
    labels.push("DEAD".into());
    for (i, label) in labels.iter().enumerate() {
        let mut p = profile();
        p.id = label.clone();
        p.name = label.clone();
        p.serial = i as u64 + 1;
        p.proxy_id = if label == "DIRECT" {
            String::new()
        } else {
            label.clone()
        };
        workspace.profiles.push(p);
    }
    let mut manager = BrowserManager::default();
    let mut reports = Vec::new();
    let mut owned_pids = Vec::new();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for batch in labels.chunks(4) {
            for label in batch {
                manager
                    .launch(
                        root.path(),
                        &workspace,
                        label,
                        true,
                        Some(&format!("{origin}/{label}")),
                    )
                    .unwrap();
                owned_pids.push(manager.instances[label].child.id());
            }
            let deadline = Instant::now() + Duration::from_secs(35);
            let mut waiting = batch.to_vec();
            while !waiting.is_empty() && Instant::now() < deadline {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(report) => {
                        let label = report["label"].as_str().unwrap_or("");
                        waiting.retain(|p| p != label);
                        println!(
                            "PROXY_QA {}",
                            json!({"label":label,"http":report["http"]["status"],"https":report["https"]["status"],"rtcCandidates":report["webrtc"]["candidates"].as_array().map(Vec::len)})
                        );
                        reports.push(report);
                    }
                    Err(_) => break,
                }
            }
            for label in waiting {
                reports.push(json!({"label":label,"error":"browser-report-timeout"}));
            }
            for label in batch {
                manager.stop(label).unwrap();
            }
            let deadline = Instant::now() + Duration::from_secs(15);
            while !manager.instances.is_empty() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(100));
                manager.reap().unwrap();
            }
            assert!(
                manager.instances.is_empty(),
                "QA browser did not close gracefully"
            );
        }
    }));
    for label in manager.instances.keys().cloned().collect::<Vec<_>>() {
        let _ = manager.stop(&label);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while !manager.instances.is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
        let _ = manager.reap();
    }
    let graceful = manager.instances.is_empty();
    for instance in manager.instances.values_mut() {
        let _ = instance.child.kill();
        let _ = instance.child.wait();
    }
    manager.instances.clear();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let evidence = json!({"date":chrono::Utc::now().to_rfc3339(),"kernel":"159.0-beta.42","realBrowserManager":true,"maxConcurrentProfiles":4,"nodes":reports,"cleanup":{"graceful":graceful,"ownedPids":owned_pids,"activeManagedBrowsers":manager.instances.len(),"temporaryProfilesRemoved":true}});
    let output =
        std::env::var("STARPERSONA_PROXY_EVIDENCE").expect("set STARPERSONA_PROXY_EVIDENCE");
    fs::write(output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error);
    }
    assert_eq!(reports.len(), labels.len());
    for report in &reports {
        let candidates = report["webrtc"]["candidates"]
            .as_array()
            .expect("ICE report missing");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate["type"] == "host"
                    || candidate["type"] == "srflx"
                    || candidate["type"] == "prflx"
                    || candidate["type"] == "relay"),
            "invalid ICE candidate included"
        );
    }
    assert!(graceful);
}
