use percent_encoding::percent_decode_str;
use url::Url;

use super::LinkParser;
use crate::error::{Error, Result};
use crate::models::{ProxyKind, ProxyNode, ServerEntry, Transport, TrojanConfig};

pub struct TrojanParser;

impl LinkParser for TrojanParser {
    fn can_parse(&self, uri: &str) -> bool {
        uri.to_ascii_lowercase().starts_with("trojan://")
    }

    fn parse(&self, uri: &str) -> Result<ServerEntry> {
        parse_trojan(uri)
    }
}

pub fn parse_trojan(uri: &str) -> Result<ServerEntry> {
    let lower_uri = uri.trim();
    if !lower_uri.to_ascii_lowercase().starts_with("trojan://") {
        return Err(Error::Parse("not a trojan URI".into()));
    }

    let parsed = Url::parse(lower_uri)
        .map_err(|e| Error::Parse(format!("invalid trojan url: {e}")))?;

    let raw_userinfo = parsed.username();
    let password = if raw_userinfo.is_empty() {
        return Err(Error::Parse("trojan URL missing password".into()));
    } else {
        percent_decode_str(raw_userinfo)
            .decode_utf8()
            .map_err(|_| Error::Parse("invalid utf-8 in userinfo".into()))?
            .into_owned()
    };

    let host_str = parsed
        .host_str()
        .ok_or_else(|| Error::Parse("trojan URL missing host".into()))?;

    let server = if host_str.starts_with('[') && host_str.ends_with(']') {
        host_str[1..host_str.len() - 1].to_string()
    } else {
        host_str.to_string()
    };

    if server.is_empty() {
        return Err(Error::Parse("trojan URL host is empty".into()));
    }

    let port = parsed.port().unwrap_or(443);

    let mut sni = String::new();
    let mut insecure = false;
    let mut alpn = Vec::new();
    let mut transport_type = "tcp".to_string();
    let mut path = "/".to_string();
    let mut host = String::new();
    let mut service_name = String::new();

    for (k, v) in parsed.query_pairs() {
        let k_lower = k.to_ascii_lowercase();
        match k_lower.as_str() {
            "sni" | "peer" => {
                sni = v.into_owned();
            }
            "insecure" | "allowinsecure" => {
                let v_lower = v.to_ascii_lowercase();
                insecure = v_lower == "1" || v_lower == "true";
            }
            "alpn" => {
                if !v.is_empty() {
                    alpn = v
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
            "type" => {
                transport_type = v.to_ascii_lowercase();
            }
            "path" => {
                path = v.into_owned();
            }
            "host" => {
                host = v.into_owned();
            }
            "servicename" => {
                service_name = v.into_owned();
            }
            _ => {}
        }
    }

    if sni.is_empty() && !server.parse::<std::net::IpAddr>().is_ok() {
        sni = server.clone();
    }

    let transport = match transport_type.as_str() {
        "ws" => Transport::Ws { path, host },
        "grpc" => Transport::Grpc { service_name },
        "httpupgrade" => Transport::Httpupgrade { path, host },
        _ => Transport::Tcp,
    };

    let name = parsed
        .fragment()
        .map(percent_decode_str)
        .map(|dec| dec.decode_utf8_lossy().into_owned())
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
        kind: ProxyKind::Trojan(TrojanConfig {
            password,
            sni,
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
    fn parse_valid_trojan() {
        let uri = "trojan://secret_pwd@trojan.example.com:443?sni=trojan.example.com&alpn=h2,http/1.1#Trojan%20Server";
        let node = parse_trojan(uri).unwrap();
        assert_eq!(node.name, "Trojan Server");
        assert_eq!(node.server, "trojan.example.com");
        assert_eq!(node.port, 443);
        let ProxyKind::Trojan(t) = node.kind else { panic!("expected Trojan") };
        assert_eq!(t.password, "secret_pwd");
        assert_eq!(t.sni, "trojan.example.com");
        assert_eq!(t.alpn, vec!["h2", "http/1.1"]);
    }
}
