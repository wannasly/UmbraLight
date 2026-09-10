use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use serde::Deserialize;

use super::LinkParser;
use crate::error::{Error, Result};
use crate::models::{ProxyKind, ProxyNode, ServerEntry, Transport, VMessConfig};

pub struct VMessParser;

impl LinkParser for VMessParser {
    fn can_parse(&self, uri: &str) -> bool {
        uri.to_ascii_lowercase().starts_with("vmess://")
    }

    fn parse(&self, uri: &str) -> Result<ServerEntry> {
        parse_vmess(uri)
    }
}

#[derive(Deserialize)]
struct RawVMess {
    #[serde(default)]
    ps: Option<String>,
    #[serde(default)]
    add: Option<String>,
    #[serde(default)]
    port: Option<serde_json::Value>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    aid: Option<serde_json::Value>,
    #[serde(default)]
    scy: Option<String>,
    #[serde(default)]
    net: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] type_: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    tls: Option<String>,
    #[serde(default)]
    sni: Option<String>,
    #[serde(default)]
    alpn: Option<String>,
}

fn parse_port(val: Option<serde_json::Value>) -> Option<u16> {
    match val? {
        serde_json::Value::Number(n) => n.as_u64().map(|p| p as u16),
        serde_json::Value::String(s) => s.trim().parse::<u16>().ok(),
        _ => None,
    }
}

fn parse_aid(val: Option<serde_json::Value>) -> u32 {
    match val {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0) as u32,
        Some(serde_json::Value::String(s)) => s.trim().parse::<u32>().unwrap_or(0),
        _ => 0,
    }
}

pub fn parse_vmess(uri: &str) -> Result<ServerEntry> {
    let trimmed = uri.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("vmess://") {
        return Err(Error::Parse("not a vmess URI".into()));
    }

    let b64 = &trimmed[8..].trim();
    let b64_clean: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    let mut padded = b64_clean;
    while padded.len() % 4 != 0 {
        padded.push('=');
    }

    let bytes = STANDARD
        .decode(&padded)
        .or_else(|_| URL_SAFE.decode(&padded))
        .map_err(|e| Error::Parse(format!("invalid base64 in vmess: {e}")))?;

    let raw: RawVMess = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Parse(format!("invalid JSON in vmess payload: {e}")))?;

    let server = raw.add.filter(|s| !s.trim().is_empty())
        .ok_or_else(|| Error::Parse("vmess missing 'add' (server)".into()))?;
    let port = parse_port(raw.port)
        .ok_or_else(|| Error::Parse("vmess missing or invalid 'port'".into()))?;
    let uuid = raw.id.filter(|s| !s.trim().is_empty())
        .ok_or_else(|| Error::Parse("vmess missing 'id' (uuid)".into()))?;

    let name = raw.ps.filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{server}:{port}"));

    let alter_id = parse_aid(raw.aid);
    let security = raw.scy.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "auto".into());

    let net = raw.net.unwrap_or_default().to_ascii_lowercase();
    let path = raw.path.unwrap_or_else(|| "/".into());
    let host = raw.host.unwrap_or_default();

    let transport = match net.as_str() {
        "ws" => Transport::Ws { path, host },
        "grpc" => Transport::Grpc { service_name: path },
        "httpupgrade" => Transport::Httpupgrade { path, host },
        _ => Transport::Tcp,
    };

    let tls = match raw.tls.as_deref() {
        Some("tls") => true,
        _ => false,
    };

    let sni = raw.sni.unwrap_or_default();
    let alpn = raw.alpn.map(|a| {
        a.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }).unwrap_or_default();

    Ok(ProxyNode {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        server,
        port,
        last_ping_ms: None,
        favorite: false,
        total_up: 0,
        total_down: 0,
        raw: uri.to_string(),
        kind: ProxyKind::VMess(VMessConfig {
            uuid,
            alter_id,
            security,
            transport,
            tls,
            sni,
            insecure: false,
            alpn,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_vmess_ws() {
        let json_str = r#"{
            "v": "2",
            "ps": "VMess Server",
            "add": "vmess.example.com",
            "port": 443,
            "id": "12345678-1234-1234-1234-123456789abc",
            "aid": 0,
            "scy": "auto",
            "net": "ws",
            "type": "none",
            "host": "cdn.example.com",
            "path": "/ws-path",
            "tls": "tls",
            "sni": "vmess.example.com"
        }"#;
        let b64 = STANDARD.encode(json_str);
        let uri = format!("vmess://{b64}");

        let node = parse_vmess(&uri).unwrap();
        assert_eq!(node.name, "VMess Server");
        assert_eq!(node.server, "vmess.example.com");
        assert_eq!(node.port, 443);
        let ProxyKind::VMess(v) = node.kind else { panic!("expected VMess") };
        assert_eq!(v.uuid, "12345678-1234-1234-1234-123456789abc");
        assert_eq!(v.security, "auto");
        assert!(v.tls);
        assert_eq!(v.sni, "vmess.example.com");
        match v.transport {
            Transport::Ws { path, host } => {
                assert_eq!(path, "/ws-path");
                assert_eq!(host, "cdn.example.com");
            }
            _ => panic!("expected ws transport"),
        }
    }
}
