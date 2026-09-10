use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use percent_encoding::percent_decode_str;

use super::LinkParser;
use crate::error::{Error, Result};
use crate::models::{ProxyKind, ProxyNode, ServerEntry, ShadowsocksConfig};

pub struct ShadowsocksParser;

impl LinkParser for ShadowsocksParser {
    fn can_parse(&self, uri: &str) -> bool {
        uri.to_ascii_lowercase().starts_with("ss://")
    }

    fn parse(&self, uri: &str) -> Result<ServerEntry> {
        parse_shadowsocks(uri)
    }
}

fn b64_decode(input: &str) -> Option<String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut padded = clean;
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    STANDARD
        .decode(&padded)
        .or_else(|_| URL_SAFE.decode(&padded))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

pub fn parse_shadowsocks(uri: &str) -> Result<ServerEntry> {
    let trimmed = uri.trim();
    if !trimmed.to_ascii_lowercase().starts_with("ss://") {
        return Err(Error::Parse("not a shadowsocks link".into()));
    }

    let after_scheme = &trimmed[5..];
    let (body_and_query, remark) = match after_scheme.split_once('#') {
        Some((bq, r)) => (bq, Some(r)),
        None => (after_scheme, None),
    };

    let (main_part, query) = match body_and_query.split_once('?') {
        Some((m, q)) => (m, Some(q)),
        None => (body_and_query, None),
    };

    let name = remark
        .map(percent_decode_str)
        .map(|dec| dec.decode_utf8_lossy().into_owned())
        .filter(|n| !n.trim().is_empty());

    let (userinfo, host_port) = if let Some((u, hp)) = main_part.split_once('@') {
        (u.to_string(), hp.to_string())
    } else {
        // Format B (SIP002 full base64)
        let decoded = b64_decode(main_part)
            .ok_or_else(|| Error::Parse("failed to base64 decode shadowsocks URI".into()))?;
        let (u, hp) = decoded
            .split_once('@')
            .ok_or_else(|| Error::Parse("missing '@' in decoded shadowsocks URI".into()))?;
        (u.to_string(), hp.to_string())
    };

    // Userinfo might be base64-encoded "method:password"
    let (method, password) = if let Some(decoded_ui) = b64_decode(&userinfo) {
        if let Some((m, p)) = decoded_ui.split_once(':') {
            (m.to_string(), p.to_string())
        } else {
            userinfo
                .split_once(':')
                .map(|(m, p)| (m.to_string(), p.to_string()))
                .ok_or_else(|| Error::Parse("invalid shadowsocks userinfo (method:password)".into()))?
        }
    } else {
        userinfo
            .split_once(':')
            .map(|(m, p)| (m.to_string(), p.to_string()))
            .ok_or_else(|| Error::Parse("invalid shadowsocks userinfo (method:password)".into()))?
    };

    let (host, port_str) = host_port
        .rsplit_once(':')
        .ok_or_else(|| Error::Parse("missing port in shadowsocks server".into()))?;

    let server = host.trim_start_matches('[').trim_end_matches(']').to_string();
    let port = port_str
        .parse::<u16>()
        .map_err(|_| Error::Parse("invalid port in shadowsocks server".into()))?;

    let mut plugin = None;
    let mut plugin_opts = None;

    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k.eq_ignore_ascii_case("plugin") {
                    let decoded_plugin = percent_decode_str(v).decode_utf8_lossy().into_owned();
                    if let Some((p, opts)) = decoded_plugin.split_once(';') {
                        plugin = Some(p.to_string());
                        plugin_opts = Some(opts.to_string());
                    } else {
                        plugin = Some(decoded_plugin);
                    }
                }
            }
        }
    }

    let node_name = name.unwrap_or_else(|| format!("{server}:{port}"));

    Ok(ProxyNode {
        id: uuid::Uuid::new_v4().to_string(),
        name: node_name,
        server,
        port,
        last_ping_ms: None,
        favorite: false,
        total_up: 0,
        total_down: 0,
        raw: uri.to_string(),
        kind: ProxyKind::Shadowsocks(ShadowsocksConfig {
            method,
            password,
            plugin,
            plugin_opts,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ss_format_a_base64_userinfo() {
        // "aes-256-gcm:secret_password" in base64 -> YWVzLTI1Ni1nY206c2VjcmV0X3Bhc3N3b3Jk
        let uri = "ss://YWVzLTI1Ni1nY206c2VjcmV0X3Bhc3N3b3Jk@ss.example.com:8388#SS%20Node";
        let node = parse_shadowsocks(uri).unwrap();
        assert_eq!(node.name, "SS Node");
        assert_eq!(node.server, "ss.example.com");
        assert_eq!(node.port, 8388);
        let ProxyKind::Shadowsocks(s) = node.kind else { panic!("expected Shadowsocks") };
        assert_eq!(s.method, "aes-256-gcm");
        assert_eq!(s.password, "secret_password");
    }

    #[test]
    fn parse_ss_format_b_sip002() {
        // "aes-128-gcm:pwd@1.2.3.4:8388" in base64 -> YWVzLTEyOC1nY206cHdkQDEuMi4zLjQ6ODM4OA==
        let uri = "ss://YWVzLTEyOC1nY206cHdkQDEuMi4zLjQ6ODM4OA==#SIP002";
        let node = parse_shadowsocks(uri).unwrap();
        assert_eq!(node.name, "SIP002");
        assert_eq!(node.server, "1.2.3.4");
        assert_eq!(node.port, 8388);
        let ProxyKind::Shadowsocks(s) = node.kind else { panic!("expected Shadowsocks") };
        assert_eq!(s.method, "aes-128-gcm");
        assert_eq!(s.password, "pwd");
    }
}
