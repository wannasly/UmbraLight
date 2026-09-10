pub mod hysteria2;
pub mod shadowsocks;
pub mod trojan;
pub mod vless;
pub mod vmess;

use std::collections::HashSet;
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use percent_encoding::percent_decode_str;

use crate::error::{Error, Result};
use crate::models::ServerEntry;

pub trait LinkParser {
    fn can_parse(&self, uri: &str) -> bool;
    fn parse(&self, uri: &str) -> Result<ServerEntry>;
}

/// Dispatch a single share link by its scheme prefix.
pub fn parse_any(uri: &str) -> Result<ServerEntry> {
    let parsers: [&dyn LinkParser; 5] = [
        &vless::VlessParser,
        &hysteria2::Hysteria2Parser,
        &vmess::VMessParser,
        &trojan::TrojanParser,
        &shadowsocks::ShadowsocksParser,
    ];

    for parser in parsers {
        if parser.can_parse(uri) {
            return parser.parse(uri);
        }
    }
    let scheme = uri.split("://").next().unwrap_or(uri);
    Err(Error::Unsupported(format!("scheme \"{scheme}\"")))
}

/// Helper to decode base64 strings with padding repair and both standard and url-safe formats.
pub fn decode_base64_text(text: &str) -> Option<String> {
    let trimmed = text.trim_start_matches('\u{feff}').trim();
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let mut padded = compact;
    while padded.len() % 4 != 0 {
        padded.push('=');
    }

    for attempt in [STANDARD.decode(&padded), URL_SAFE.decode(&padded)] {
        if let Ok(bytes) = attempt {
            if let Ok(decoded) = String::from_utf8(bytes) {
                return Some(decoded);
            }
        }
    }
    None
}

/// Parse a blob of text containing one or multiple links separated by whitespace/newlines.
pub fn parse_links(text: &str) -> (Vec<ServerEntry>, Vec<String>) {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut servers = Vec::new();
    let mut errors = Vec::new();

    for token in text.split_whitespace() {
        let token = token.trim_start_matches('\u{feff}');
        if !token.contains("://") {
            continue;
        }
        if !seen.insert(token) {
            continue;
        }
        match parse_any(token) {
            Ok(entry) => servers.push(entry),
            Err(e) => errors.push(format!("{}: {e}", label(token))),
        }
    }
    (servers, errors)
}

fn label(link: &str) -> String {
    let name = link
        .split_once('#')
        .map(|(_, frag)| percent_decode_str(frag).decode_utf8_lossy().into_owned())
        .unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        short(link)
    } else {
        short(name)
    }
}

fn short(link: &str) -> String {
    const MAX: usize = 64;
    if link.chars().count() <= MAX {
        link.to_string()
    } else {
        let mut s: String = link.chars().take(MAX).collect();
        s.push('\u{2026}');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_links_dedupes_and_skips_bad() {
        let text = "vless://u1@h.com:443?security=none#a\n\
                    vless://u1@h.com:443?security=none#a\n\
                    unknown://something#bad\n\
                    not-a-link";
        let (servers, errors) = parse_links(text);
        assert_eq!(servers.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("unknown"));
    }

    #[test]
    fn decode_base64_handles_padding() {
        let text = "dmxlc3M6Ly91MUBoLmNvbTo0NDM/c2VjdXJpdHk9bm9uZSNh";
        let decoded = decode_base64_text(text).unwrap();
        assert!(decoded.starts_with("vless://"));
    }
}
