use std::collections::{HashMap, HashSet};
use std::time::Duration;

use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use percent_encoding::percent_decode_str;

use crate::error::{Error, Result};
use crate::models::{ServerEntry, SubscriptionQuota};
use crate::parser;

pub const DEFAULT_SUB_USER_AGENT: &str = "Happ/2.0.0 UmbraLight/1.0.0";

/// Preserve local server state across subscription refreshes and format changes.
pub fn merge_servers(existing: &[ServerEntry], fetched: Vec<ServerEntry>) -> Vec<ServerEntry> {
    let mut claimed = HashSet::new();
    let mut seen = HashSet::new();
    let mut merged = Vec::with_capacity(fetched.len());
    for mut server in fetched {
        if !seen.insert(server.raw.clone()) {
            continue;
        }
        let match_index = existing
            .iter()
            .enumerate()
            .find_map(|(i, old)| (!claimed.contains(&i) && old.raw == server.raw).then_some(i))
            .or_else(|| {
                existing.iter().enumerate().find_map(|(i, old)| {
                    (!claimed.contains(&i) && endpoint_identity(old) == endpoint_identity(&server))
                        .then_some(i)
                })
            });
        if let Some(i) = match_index {
            claimed.insert(i);
            let old = &existing[i];
            server.id = old.id.clone();
            server.favorite = old.favorite;
            server.last_ping_ms = old.last_ping_ms;
            server.total_up = old.total_up;
            server.total_down = old.total_down;
        }
        merged.push(server);
    }
    merged
}

fn endpoint_identity(s: &ServerEntry) -> String {
    use crate::models::ProxyKind;
    let credentials = match &s.kind {
        ProxyKind::Vless(v) => format!("vless:{}:{:?}", v.uuid, v.transport),
        ProxyKind::Hysteria2(h) => format!("hy2:{}", h.password),
        ProxyKind::VMess(v) => format!("vmess:{}:{:?}", v.uuid, v.transport),
        ProxyKind::Trojan(t) => format!("trojan:{}:{:?}", t.password, t.transport),
        ProxyKind::Shadowsocks(ss) => format!("ss:{}:{}", ss.method, ss.password),
    };
    format!("{}:{}:{credentials}", s.server.to_ascii_lowercase(), s.port)
}
const H_HWID_NOT_SUPPORTED: &str = "x-hwid-not-supported";
const H_HWID_MAX_DEVICES: &str = "x-hwid-max-devices-reached";
const H_PROFILE_TITLE: &str = "profile-title";
const H_UPDATE_INTERVAL: &str = "profile-update-interval";
const H_SUPPORT_URL: &str = "support-url";
const H_WEB_PAGE_URL: &str = "profile-web-page-url";

#[derive(Debug, Clone)]
pub struct FetchedSubscription {
    pub servers: Vec<ServerEntry>,
    pub errors: Vec<String>,
    pub quota: Option<SubscriptionQuota>,
    pub filename: Option<String>,
    pub title: Option<String>,
    pub update_interval_hours: Option<u32>,
    pub support_url: Option<String>,
    pub web_page_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceIdentity {
    pub hwid: String,
    pub os: String,
    pub os_version: String,
    pub model: String,
}

pub async fn fetch_subscription(
    url: &str,
    user_agent: Option<&str>,
    identity: Option<&DeviceIdentity>,
) -> Result<FetchedSubscription> {
    let ua = match user_agent {
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => DEFAULT_SUB_USER_AGENT,
    };

    let client = reqwest::Client::builder()
        .no_proxy()
        .user_agent(ua)
        .timeout(Duration::from_secs(20))
        .build()?;

    let mut request = client.get(url);
    if let Some(id) = identity {
        if !id.hwid.is_empty() {
            request = request.header("x-hwid", &id.hwid);
        }
        if !id.os.is_empty() {
            request = request.header("x-device-os", &id.os);
        }
        if !id.os_version.is_empty() {
            request = request.header("x-ver-os", &id.os_version);
        }
        if !id.model.is_empty() {
            request = request.header("x-device-model", &id.model);
        }
    }

    let resp = request.send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(Error::Network(format!(
            "subscription server returned HTTP {status}"
        )));
    }

    let gate = hwid_rejection(resp.headers());
    let quota = resp
        .headers()
        .get("subscription-userinfo")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_userinfo);

    let filename = resp
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_disposition);

    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };

    let title = header(H_PROFILE_TITLE).and_then(|v| decode_header_text(&v));
    let update_interval_hours = header(H_UPDATE_INTERVAL).and_then(|v| v.parse::<u32>().ok());
    let support_url = header(H_SUPPORT_URL).filter(|u| is_http_url(u));
    let web_page_url = header(H_WEB_PAGE_URL).filter(|u| is_http_url(u));

    let body = resp.text().await?;
    let trimmed = body.trim_start_matches('\u{feff}').trim();
    let (parsed, errors) = if trimmed.starts_with('[') {
        parser::v2ray_json::parse_v2ray_json(trimmed)
    } else {
        let list = decode_body(&body)?;
        parser::parse_links(&list)
    };
    let servers = drop_placeholders(parsed, gate)?;

    Ok(FetchedSubscription {
        servers,
        errors,
        quota,
        filename,
        title,
        update_interval_hours,
        support_url,
        web_page_url,
    })
}

pub fn decode_header_text(value: &str) -> Option<String> {
    let Some(payload) = value.strip_prefix("base64:") else {
        return Some(value.trim().to_string()).filter(|v| !v.is_empty());
    };
    let payload: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    let mut padded = payload;
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    for attempt in [STANDARD.decode(&padded), URL_SAFE.decode(&padded)] {
        if let Ok(bytes) = attempt {
            if let Ok(text) = String::from_utf8(bytes) {
                let text = text.trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn drop_placeholders(parsed: Vec<ServerEntry>, gate: Option<Error>) -> Result<Vec<ServerEntry>> {
    let had_any = !parsed.is_empty();
    let servers: Vec<ServerEntry> = parsed.into_iter().filter(|s| !is_placeholder(s)).collect();
    if !servers.is_empty() {
        return Ok(servers);
    }
    if let Some(err) = gate {
        return Err(err);
    }
    if had_any {
        return Err(Error::DeviceLimit);
    }
    Ok(servers)
}

fn hwid_rejection(headers: &reqwest::header::HeaderMap) -> Option<Error> {
    if header_flag(headers, H_HWID_MAX_DEVICES) {
        return Some(Error::DeviceLimit);
    }
    if header_flag(headers, H_HWID_NOT_SUPPORTED) {
        return Some(Error::HwidRequired);
    }
    None
}

fn header_flag(headers: &reqwest::header::HeaderMap, name: &str) -> bool {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .is_some_and(|v| !v.is_empty() && !v.eq_ignore_ascii_case("false") && v != "0")
}

pub fn is_placeholder(s: &ServerEntry) -> bool {
    s.port == 1 && matches!(s.server.as_str(), "0.0.0.0" | "::" | "127.0.0.1")
}

pub fn decode_body(body: &str) -> Result<String> {
    let trimmed = body.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(Error::Parse("subscription response is empty".into()));
    }
    if trimmed.lines().next().is_some_and(|l| l.contains("://")) {
        return Ok(trimmed.to_string());
    }

    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let mut padded = compact;
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    for attempt in [STANDARD.decode(&padded), URL_SAFE.decode(&padded)] {
        if let Ok(bytes) = attempt {
            let decoded = String::from_utf8_lossy(&bytes);
            if decoded.contains("://") {
                return Ok(decoded.into_owned());
            }
        }
    }

    if trimmed.starts_with("proxies:") || trimmed.contains("\nproxies:") {
        return Err(Error::Unsupported(
            "Clash YAML subscription (URI list expected)".into(),
        ));
    }
    if trimmed.starts_with('{') {
        return Err(Error::Unsupported(
            "JSON config subscription (URI list expected)".into(),
        ));
    }
    Err(Error::Parse("unrecognized subscription format".into()))
}

pub fn parse_userinfo(value: &str) -> Option<SubscriptionQuota> {
    let mut map: HashMap<String, u64> = HashMap::new();
    for part in value.split(';') {
        if let Some((k, v)) = part.trim().split_once('=') {
            if let Ok(n) = v.trim().parse::<f64>() {
                map.insert(k.trim().to_ascii_lowercase(), n.max(0.0) as u64);
            }
        }
    }
    if map.is_empty() {
        return None;
    }
    Some(SubscriptionQuota {
        upload: map.get("upload").copied().unwrap_or(0),
        download: map.get("download").copied().unwrap_or(0),
        total: map.get("total").copied().unwrap_or(0),
        expire: map.get("expire").copied().unwrap_or(0),
    })
}

fn filename_from_disposition(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            let rest = rest.split_once("''").map(|(_, v)| v).unwrap_or(rest);
            let name = percent_decode_str(rest).decode_utf8_lossy().into_owned();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename=") {
            let name = rest.trim_matches('"').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_userinfo_valid() {
        let val = "upload=1024; download=2048; total=10485760; expire=1735689600";
        let quota = parse_userinfo(val).unwrap();
        assert_eq!(quota.upload, 1024);
        assert_eq!(quota.download, 2048);
        assert_eq!(quota.total, 10485760);
        assert_eq!(quota.expire, 1735689600);
    }

    #[test]
    fn decode_body_base64() {
        let raw = "vless://u1@1.2.3.4:443?security=none#Server1\nvless://u2@5.6.7.8:443?security=none#Server2";
        let b64 = STANDARD.encode(raw);
        let decoded = decode_body(&b64).unwrap();
        assert!(decoded.contains("Server1"));
        assert!(decoded.contains("Server2"));
    }

    #[test]
    fn decode_header_base64_title() {
        let title_b64 = "base64:0JzQvtGPINCf0L7QtNC/0LjRgdC60LA=";
        let title = decode_header_text(title_b64).unwrap();
        assert_eq!(title, "Моя Подписка");
    }

    #[test]
    fn refresh_preserves_local_state_and_distinct_nodes() {
        let old_uri =
            "vless://11111111-2222-3333-4444-555555555555@example.com:443?security=none#Old";
        let new_uri =
            "vless://11111111-2222-3333-4444-555555555555@example.com:443?security=none#New";
        let (mut old, _) = parser::parse_links(old_uri);
        old[0].favorite = true;
        old[0].last_ping_ms = Some(42);
        let original_id = old[0].id.clone();
        let (mut fetched, _) = parser::parse_links(new_uri);
        let mut second = fetched[0].clone();
        second.id = "second".into();
        second.raw.push_str("-backup");
        fetched.push(second);
        let merged = merge_servers(&old, fetched);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, original_id);
        assert!(merged[0].favorite);
        assert_eq!(merged[0].last_ping_ms, Some(42));
        assert_eq!(merged[1].id, "second");
    }

    #[tokio::test]
    async fn fetches_v2ray_json_subscription_over_http() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = serde_json::json!([{
            "remarks": "JSON node",
            "outbounds": [{
                "tag": "proxy", "protocol": "vless",
                "settings": {"vnext": [{"address": "example.com", "port": 443,
                    "users": [{"id": "11111111-2222-3333-4444-555555555555"}]}]},
                "streamSettings": {"network": "tcp", "security": "none"}
            }]
        }])
        .to_string();
        let serve = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 2048];
            let size = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_string();
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        let fetched = fetch_subscription(&format!("http://127.0.0.1:{port}/sub"), None, None)
            .await
            .unwrap();
        assert_eq!(fetched.servers.len(), 1);
        assert_eq!(fetched.servers[0].name, "JSON node");
        assert!(serve.await.unwrap().contains(DEFAULT_SUB_USER_AGENT));
    }
}
