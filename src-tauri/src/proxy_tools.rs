use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    process::Command,
};

const STATUS: &str = "http://127.0.0.1:18765/api/status?nodeIds=1";
const PING: &str = "https://cp.cloudflare.com/generate_204";
const SPEED: &str = "https://speed.cloudflare.com/__down?bytes=1048576";
fn curl() -> Command {
    #[cfg(unix)]
    let path = "/usr/bin/curl";
    #[cfg(windows)]
    let path = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("System32/curl.exe");
    let mut c = Command::new(path);
    c.args(["-q", "--silent", "--globoff", "--connect-timeout", "4"]);
    c
}
#[derive(Deserialize)]
struct Exit {
    name: String,
    port: u16,
    open: bool,
    socks5h: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    pending: bool,
    exits: Vec<Exit>,
    node_ids: Option<BTreeMap<String, String>>,
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Candidate {
    name: String,
    port: u16,
    node_id: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct Listing {
    nodes: Vec<Candidate>,
    skipped: usize,
}
fn decode(body: &[u8]) -> Result<Listing, String> {
    let s: Status = serde_json::from_slice(body).map_err(|_| "StarPort 列表格式不正确")?;
    if s.pending {
        return Err(
            "StarPort 有尚未应用的端口变更，请先在星口应用，再导入。现有代理未改动。".into(),
        );
    }
    if s.exits.len() > 5000 {
        return Err("StarPort 列表过大".into());
    }
    let mut ports = HashSet::new();
    let mut nodes = vec![];
    let total = s.exits.len();
    for e in s.exits {
        if !ports.insert(e.port) {
            return Err("StarPort 返回了重复的监听端口，请检查服务".into());
        }
        if !e.open
            || e.port < 1024
            || e.socks5h != format!("socks5h://127.0.0.1:{}", e.port)
            || e.name.trim().is_empty()
            || e.name.encode_utf16().count() > 80
            || e.name.chars().any(char::is_control)
        {
            continue;
        }
        let node_id = if let Some(ids) = &s.node_ids {
            match ids
                .get(&e.port.to_string())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
            {
                Some(id) => Some(id.to_string()),
                None => continue,
            }
        } else {
            None
        };
        nodes.push(Candidate {
            name: e.name,
            port: e.port,
            node_id,
        });
    }
    Ok(Listing {
        skipped: total - nodes.len(),
        nodes,
    })
}
fn list() -> Result<Listing, String> {
    let o = curl()
        .args([
            "--noproxy",
            "*",
            "--fail",
            "--max-time",
            "12",
            "--max-filesize",
            "2097152",
            STATUS,
        ])
        .output()
        .map_err(|_| "找不到系统 curl")?;
    if !o.status.success() {
        return Err(
            "无法读取本机 StarPort (127.0.0.1:18765)，请检查服务是否已启动。原代理未改动。".into(),
        );
    }
    decode(&o.stdout)
}
#[tauri::command]
pub(crate) async fn starport_list() -> Result<Listing, String> {
    tauri::async_runtime::spawn_blocking(list)
        .await
        .map_err(|_| "读取任务中断")?
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Measure {
    pub ms: u64,
    pub bytes_per_second: Option<u64>,
}
fn measure(protocol: &str, host: &str, port: u16, speed: bool) -> Result<Measure, String> {
    let scheme = match protocol {
        "SOCKS5" => "socks5h",
        "HTTP" => "http",
        _ => return Err("不支持的代理协议".into()),
    };
    if port == 0
        || host.is_empty()
        || host.len() > 253
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err("无效代理地址".into());
    }
    if ["192.0.2.", "198.51.100.", "203.0.113."]
        .iter()
        .any(|s| host.starts_with(s))
    {
        return Err("示例地址不参与检测".into());
    }
    let proxy = format!("{scheme}://{host}:{port}");
    let o = curl()
        .args([
            "--proto",
            "=https",
            "--noproxy",
            "",
            "--proxy",
            &proxy,
            "--max-time",
            if speed { "15" } else { "6" },
            "--max-filesize",
            if speed { "1048576" } else { "1024" },
            "--output",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
            "--write-out",
            "%{http_code} %{size_download} %{time_total}",
            if speed { SPEED } else { PING },
        ])
        .output()
        .map_err(|_| "找不到系统 curl")?;
    if !o.status.success() {
        return Err(match o.status.code() {
            Some(28) => "超时",
            Some(7) => "代理连接失败",
            Some(60) => "TLS 证书验证失败",
            _ => "代理请求失败",
        }
        .into());
    }
    parse_measure(&String::from_utf8_lossy(&o.stdout), speed)
}
fn parse_measure(s: &str, speed: bool) -> Result<Measure, String> {
    let fields = s.split_whitespace().collect::<Vec<_>>();
    let (code, bytes) = if speed {
        ("200", "1048576")
    } else {
        ("204", "0")
    };
    if fields.len() != 3 || fields[0] != code || fields[1] != bytes {
        return Err("测试站响应不符合预期".into());
    };
    let t = fields[2].parse::<f64>().map_err(|_| "无效耗时")?;
    if !t.is_finite() || t <= 0.0 {
        return Err("无效耗时".into());
    };
    Ok(Measure {
        ms: (t * 1000.).ceil() as u64,
        bytes_per_second: speed.then(|| (1048576. / t) as u64),
    })
}
#[tauri::command]
pub(crate) async fn proxy_measure(
    protocol: String,
    host: String,
    port: u16,
    speed: bool,
) -> Result<Measure, String> {
    tauri::async_runtime::spawn_blocking(move || measure(&protocol, &host, port, speed))
        .await
        .map_err(|_| "检测任务中断")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unapplied_and_nonlocal_listeners() {
        assert!(decode(br#"{"pending":true,"exits":[]}"#).is_err());
        let a=decode(br#"{"pending":false,"exits":[{"open":true,"name":"x","port":18001,"socks5h":"socks5h://outside:18001"}]}"#).unwrap();
        assert_eq!(a.skipped, 1)
    }
    #[test]
    fn speed_requires_full_payload() {
        assert!(parse_measure("200 16 0.1", true).is_err());
        assert_eq!(
            parse_measure("200 1048576 2", true)
                .unwrap()
                .bytes_per_second,
            Some(524288)
        );
        assert!(parse_measure("200 0 1", false).is_err());
        assert!(measure("HTTP", "foo@evil", 8080, false).is_err());
    }
    #[test]
    #[ignore = "requires local StarPort and performs a 1 MiB public HTTPS request"]
    fn local_starport_smoke() {
        let l = list().unwrap();
        let mut alive = None;
        for n in l.nodes.iter().take(5) {
            if let Ok(p) = measure("SOCKS5", "127.0.0.1", n.port, false) {
                alive = Some((n, p));
                break;
            }
        }
        let (n, p) = alive.expect("no responsive proxy in first five");
        let s = measure("SOCKS5", "127.0.0.1", n.port, true).unwrap();
        println!(
            "{} listed, {} ms, {} bytes/s",
            l.nodes.len(),
            p.ms,
            s.bytes_per_second.unwrap()
        );
    }
}
