use std::collections::HashMap;
use std::net::IpAddr;

use percent_encoding::percent_decode_str;
use url::Url;

use super::LinkParser;
use crate::error::{Error, Result};
use crate::models::{ProxyKind, ProxyNode, Security, ServerEntry, Transport, VlessConfig};

pub struct VlessParser;

impl LinkParser for VlessParser {
    fn can_parse(&self, uri: &str) -> bool {
        uri.to_ascii_lowercase().starts_with("vless://")
    }

    fn parse(&self, uri: &str) -> Result<ServerEntry> {
        parse_vless(uri)
    }
}

fn dec(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

/// Parse query string manually so + is kept intact (e.g. for base64 pbk).
fn query_map(query: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(query) = query else { return map };
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        map.entry(dec(k)).or_insert_with(|| v.to_string());
    }
    map
}

pub fn parse_vless(uri: &str) -> Result<ServerEntry> {
    let url = Url::parse(uri).map_err(|e| Error::Parse(format!("invalid vless link: {e}")))?;
    if url.scheme() != "vless" {
        return Err(Error::Parse("not a vless link".into()));
    }

    let uuid = dec(url.username());
    if uuid.is_empty() {
        return Err(Error::Parse("missing user id before @".into()));
    }
    let host_raw = url
        .host_str()
        .ok_or_else(|| Error::Parse("missing server address".into()))?;
    let server = host_raw
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let port = url
        .port()
        .ok_or_else(|| Error::Parse("missing port".into()))?;

    let q = query_map(url.query());
    let get = |k: &str| q.get(k).map(|v| dec(v));

    if get("headerType").as_deref() == Some("http") {
        return Err(Error::Unsupported(
            "headerType=http (HTTP obfuscation)".into(),
        ));
    }

    let type_param = get("type")
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "tcp".into());

    let transport = match type_param.as_str() {
        "tcp" => Transport::Tcp,
        "ws" | "httpupgrade" => {
            let mut path = get("path")
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "/".into());
            if let Some(i) = path.find("?ed=") {
                path.truncate(i);
            }
            if path.is_empty() {
                path = "/".into();
            }
            let host = get("host").unwrap_or_default();
            if type_param == "ws" {
                Transport::Ws { path, host }
            } else {
                Transport::Httpupgrade { path, host }
            }
        }
        "grpc" => Transport::Grpc {
            service_name: get("serviceName").unwrap_or_default(),
        },
        other => {
            let detail = match other {
                "xhttp" | "splithttp" => " - sing-box has no xhttp/SplitHTTP transport",
                _ => "",
            };
            return Err(Error::Unsupported(format!("transport \"{other}\"{detail}")));
        }
    };

    let security_param = get("security").filter(|s| !s.is_empty());
    let security = match security_param.as_deref().unwrap_or("none") {
        "reality" => Security::Reality,
        "tls" => Security::Tls,
        "none" => Security::None,
        other => {
            return Err(Error::Unsupported(format!("security \"{other}\"")));
        }
    };

    let sni = if security == Security::None {
        String::new()
    } else {
        get("sni")
            .filter(|s| !s.is_empty())
            .or_else(|| get("host").filter(|s| !s.is_empty()))
            .or_else(|| {
                if server.parse::<IpAddr>().is_err() {
                    Some(server.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    };

    let fingerprint = match security {
        Security::Reality => get("fp")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "chrome".into()),
        _ => get("fp").unwrap_or_default(),
    };

    let public_key = q.get("pbk").cloned().unwrap_or_default();
    if security == Security::Reality && public_key.is_empty() {
        return Err(Error::Parse("reality link is missing pbk".into()));
    }
    let short_id = get("sid").unwrap_or_default();

    let flow = if transport == Transport::Tcp {
        get("flow").unwrap_or_default()
    } else {
        String::new()
    };

    let alpn = if security == Security::Reality {
        Vec::new()
    } else {
        get("alpn")
            .map(|a| {
                a.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };

    let insecure = get("allowInsecure")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || get("insecure").map(|v| v == "1").unwrap_or(false);

    let name = url
        .fragment()
        .map(dec)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("{server}:{port}"));

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
        kind: ProxyKind::Vless(VlessConfig {
            uuid,
            flow,
            security,
            sni,
            fingerprint,
            public_key,
            short_id,
            insecure,
            alpn,
            transport,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reality_vision_tcp() {
        let uri = "vless://b831381d-6324-4d53-ad4f-8cda48b30811@example.com:443?security=reality&sni=yahoo.com&fp=firefox&pbk=SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc&sid=6ba85179&flow=xtls-rprx-vision&type=tcp#Moscow%20Fast";
        let s = parse_vless(uri).unwrap();
        assert_eq!(s.name, "Moscow Fast");
        assert_eq!(s.server, "example.com");
        assert_eq!(s.port, 443);
        let ProxyKind::Vless(v) = s.kind else { panic!("not vless") };
        assert_eq!(v.uuid, "b831381d-6324-4d53-ad4f-8cda48b30811");
        assert_eq!(v.security, Security::Reality);
        assert_eq!(v.sni, "yahoo.com");
        assert_eq!(v.fingerprint, "firefox");
        assert_eq!(v.public_key, "SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc");
        assert_eq!(v.short_id, "6ba85179");
        assert_eq!(v.flow, "xtls-rprx-vision");
        assert_eq!(v.transport, Transport::Tcp);
    }

    #[test]
    fn parse_ws_tls() {
        let uri = "vless://u1@host.com:8443?security=tls&type=ws&path=%2Fws%3Fed%3D2048&host=cdn.example.org&alpn=h2,http/1.1&allowInsecure=1#WS%20Node";
        let s = parse_vless(uri).unwrap();
        assert_eq!(s.name, "WS Node");
        let ProxyKind::Vless(v) = s.kind else { panic!("not vless") };
        assert_eq!(v.security, Security::Tls);
        assert!(v.insecure);
        assert_eq!(v.alpn, vec!["h2", "http/1.1"]);
        match v.transport {
            Transport::Ws { path, host } => {
                assert_eq!(path, "/ws");
                assert_eq!(host, "cdn.example.org");
            }
            _ => panic!("expected ws transport"),
        }
    }

    #[test]
    fn parse_grpc_reality() {
        let uri = "vless://u1@grpc.example.com:443?security=reality&pbk=key123&type=grpc&serviceName=mygrpc#GRPC";
        let s = parse_vless(uri).unwrap();
        let ProxyKind::Vless(v) = s.kind else { panic!("not vless") };
        match v.transport {
            Transport::Grpc { service_name } => assert_eq!(service_name, "mygrpc"),
            _ => panic!("expected grpc transport"),
        }
    }
}
