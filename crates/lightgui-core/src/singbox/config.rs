use std::collections::{HashMap, HashSet};
use serde_json::{json, Map, Value};

use crate::error::Result;
use crate::models::{
    CoreMode, DomainMatcher, ProcessMatcher, ProxyKind, ProxyNode, RouteTarget,
    RuleAction, RuleType, Security, ServerEntry, Settings, Transport,
    VlessConfig, Hysteria2Config, VMessConfig, TrojanConfig, ShadowsocksConfig,
};

const RESERVED_TAGS: [&str; 4] = ["proxy", "auto", "direct", "block"];

#[derive(Debug, Clone)]
pub struct GeneratedConfig {
    pub json: Value,
    pub tag_by_server_id: HashMap<String, String>,
    pub clash_port: u16,
    pub clash_secret: String,
}

pub fn generate(
    settings: &Settings,
    servers: &[&ServerEntry],
    selected_id: Option<&str>,
    clash_port: u16,
    clash_secret: &str,
) -> Result<GeneratedConfig> {
    let tags = assign_tags(servers);
    let tag_by_server_id: HashMap<String, String> = servers
        .iter()
        .zip(tags.iter())
        .map(|(s, t)| (s.id.clone(), t.clone()))
        .collect();

    let selected_tag = if let Some(sid) = selected_id {
        servers
            .iter()
            .position(|s| s.id == sid)
            .map(|i| tags[i].clone())
            .unwrap_or_else(|| tags.first().cloned().unwrap_or_else(|| "auto".to_string()))
    } else {
        tags.first().cloned().unwrap_or_else(|| "auto".to_string())
    };

    let mut selector_outbounds = Vec::with_capacity(tags.len() + 1);
    selector_outbounds.push(json!("auto"));
    selector_outbounds.extend(tags.iter().map(|t| json!(t)));

    let mut outbounds = Vec::with_capacity(servers.len() + 4);
    outbounds.push(json!({
        "type": "selector",
        "tag": "proxy",
        "outbounds": selector_outbounds,
        "default": selected_tag,
        "interrupt_exist_connections": true
    }));

    outbounds.push(json!({
        "type": "urltest",
        "tag": "auto",
        "outbounds": tags,
        "url": settings.ping_url,
        "interval": "3m",
        "tolerance": 50
    }));

    for (server, tag) in servers.iter().zip(tags.iter()) {
        outbounds.push(server_outbound(server, tag));
    }

    outbounds.push(json!({ "type": "direct", "tag": "direct" }));
    outbounds.push(json!({ "type": "block", "tag": "block" }));

    let config_json = json!({
        "log": {
            "level": settings.log_level,
            "timestamp": true
        },
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{clash_port}"),
                "secret": clash_secret,
                "default_mode": match settings.routing_mode {
                    crate::models::RoutingMode::Rule => "Rule",
                    crate::models::RoutingMode::GlobalProxy => "Global",
                    crate::models::RoutingMode::DirectBypass => "Direct",
                }
            },
            "cache_file": { "enabled": true, "path": "cache.db" }
        },
        "dns": {
            "servers": [
                { "tag": "dns-remote", "type": "https", "server": "1.1.1.1", "detour": "proxy" },
                { "tag": "dns-local", "type": "local" }
            ],
            "final": "dns-remote",
            "strategy": settings.ip_strategy.as_str(),
            "independent_cache": true
        },
        "inbounds": inbounds(settings),
        "outbounds": outbounds,
        "route": route(settings)
    });

    Ok(GeneratedConfig {
        json: config_json,
        tag_by_server_id,
        clash_port,
        clash_secret: clash_secret.to_string(),
    })
}

fn sanitize_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn assign_tags(servers: &[&ServerEntry]) -> Vec<String> {
    let mut used: HashSet<String> = RESERVED_TAGS.iter().map(|t| t.to_string()).collect();
    let mut tags = Vec::with_capacity(servers.len());
    for (i, server) in servers.iter().enumerate() {
        let mut base = sanitize_name(&server.name);
        if base.is_empty() {
            base = format!("server-{}", i + 1);
        }
        let mut tag = base.clone();
        let mut n = 2;
        while !used.insert(tag.clone()) {
            tag = format!("{base} ({n})");
            n += 1;
        }
        tags.push(tag);
    }
    tags
}

fn server_outbound(s: &ProxyNode, tag: &str) -> Value {
    match &s.kind {
        ProxyKind::Vless(v) => vless_outbound(s, v, tag),
        ProxyKind::Hysteria2(h) => hysteria2_outbound(s, h, tag),
        ProxyKind::VMess(vm) => vmess_outbound(s, vm, tag),
        ProxyKind::Trojan(tr) => trojan_outbound(s, tr, tag),
        ProxyKind::Shadowsocks(ss) => shadowsocks_outbound(s, ss, tag),
    }
}

fn vless_outbound(s: &ProxyNode, v: &VlessConfig, tag: &str) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), json!("vless"));
    o.insert("tag".into(), json!(tag));
    o.insert("server".into(), json!(s.server));
    o.insert("server_port".into(), json!(s.port));
    o.insert("uuid".into(), json!(v.uuid));
    if !v.flow.is_empty() {
        o.insert("flow".into(), json!(v.flow));
    }
    o.insert("packet_encoding".into(), json!("xudp"));

    if v.security != Security::None {
        let mut tls = Map::new();
        tls.insert("enabled".into(), json!(true));
        tls.insert("server_name".into(), json!(v.sni));
        tls.insert("insecure".into(), json!(v.insecure));
        if !v.fingerprint.is_empty() || v.security == Security::Reality {
            let fingerprint = if v.fingerprint.is_empty() {
                "chrome"
            } else {
                v.fingerprint.as_str()
            };
            tls.insert(
                "utls".into(),
                json!({ "enabled": true, "fingerprint": fingerprint }),
            );
        }
        if !v.alpn.is_empty() {
            tls.insert("alpn".into(), json!(v.alpn));
        }
        if v.security == Security::Reality {
            tls.insert(
                "reality".into(),
                json!({
                    "enabled": true,
                    "public_key": v.public_key,
                    "short_id": v.short_id
                }),
            );
        }
        o.insert("tls".into(), Value::Object(tls));
    }

    match &v.transport {
        Transport::Tcp => {}
        Transport::Ws { path, host } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("ws"));
            t.insert("path".into(), json!(path));
            if !host.is_empty() {
                t.insert("headers".into(), json!({ "Host": host }));
            }
            o.insert("transport".into(), Value::Object(t));
        }
        Transport::Grpc { service_name } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("grpc"));
            t.insert("service_name".into(), json!(service_name));
            o.insert("transport".into(), Value::Object(t));
        }
        Transport::Httpupgrade { path, host } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("httpupgrade"));
            t.insert("path".into(), json!(path));
            if !host.is_empty() {
                t.insert("host".into(), json!(host));
            }
            o.insert("transport".into(), Value::Object(t));
        }
    }

    Value::Object(o)
}

fn hysteria2_outbound(s: &ProxyNode, h: &Hysteria2Config, tag: &str) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), json!("hysteria2"));
    o.insert("tag".into(), json!(tag));
    o.insert("server".into(), json!(s.server));
    o.insert("server_port".into(), json!(s.port));
    o.insert("password".into(), json!(h.password));

    let mut tls = Map::new();
    tls.insert("enabled".into(), json!(true));
    if !h.sni.is_empty() {
        tls.insert("server_name".into(), json!(h.sni));
    } else {
        tls.insert("server_name".into(), json!(s.server));
    }
    tls.insert("insecure".into(), json!(h.insecure));
    if !h.alpn.is_empty() {
        tls.insert("alpn".into(), json!(h.alpn));
    }
    o.insert("tls".into(), Value::Object(tls));

    if let Some(obfs) = &h.obfs {
        match obfs {
            crate::models::Hysteria2Obfs::Salamander { password } => {
                o.insert(
                    "obfs".into(),
                    json!({
                        "type": "salamander",
                        "password": password
                    }),
                );
            }
        }
    }

    Value::Object(o)
}

fn vmess_outbound(s: &ProxyNode, vm: &VMessConfig, tag: &str) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), json!("vmess"));
    o.insert("tag".into(), json!(tag));
    o.insert("server".into(), json!(s.server));
    o.insert("server_port".into(), json!(s.port));
    o.insert("uuid".into(), json!(vm.uuid));
    o.insert("security".into(), json!(vm.security));
    o.insert("alter_id".into(), json!(vm.alter_id));
    o.insert("packet_encoding".into(), json!("xudp"));

    if vm.tls {
        let mut tls = Map::new();
        tls.insert("enabled".into(), json!(true));
        tls.insert("server_name".into(), json!(vm.sni));
        tls.insert("insecure".into(), json!(vm.insecure));
        if !vm.alpn.is_empty() {
            tls.insert("alpn".into(), json!(vm.alpn));
        }
        o.insert("tls".into(), Value::Object(tls));
    }

    match &vm.transport {
        Transport::Tcp => {}
        Transport::Ws { path, host } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("ws"));
            t.insert("path".into(), json!(path));
            if !host.is_empty() {
                t.insert("headers".into(), json!({ "Host": host }));
            }
            o.insert("transport".into(), Value::Object(t));
        }
        Transport::Grpc { service_name } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("grpc"));
            t.insert("service_name".into(), json!(service_name));
            o.insert("transport".into(), Value::Object(t));
        }
        Transport::Httpupgrade { path, host } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("httpupgrade"));
            t.insert("path".into(), json!(path));
            if !host.is_empty() {
                t.insert("host".into(), json!(host));
            }
            o.insert("transport".into(), Value::Object(t));
        }
    }

    Value::Object(o)
}

fn trojan_outbound(s: &ProxyNode, tr: &TrojanConfig, tag: &str) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), json!("trojan"));
    o.insert("tag".into(), json!(tag));
    o.insert("server".into(), json!(s.server));
    o.insert("server_port".into(), json!(s.port));
    o.insert("password".into(), json!(tr.password));

    let mut tls = Map::new();
    tls.insert("enabled".into(), json!(true));
    let sni = if !tr.sni.is_empty() {
        tr.sni.clone()
    } else {
        s.server.clone()
    };
    tls.insert("server_name".into(), json!(sni));
    tls.insert("insecure".into(), json!(tr.insecure));
    if !tr.alpn.is_empty() {
        tls.insert("alpn".into(), json!(tr.alpn));
    }
    o.insert("tls".into(), Value::Object(tls));

    match &tr.transport {
        Transport::Tcp => {}
        Transport::Ws { path, host } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("ws"));
            t.insert("path".into(), json!(path));
            if !host.is_empty() {
                t.insert("headers".into(), json!({ "Host": host }));
            }
            o.insert("transport".into(), Value::Object(t));
        }
        Transport::Grpc { service_name } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("grpc"));
            t.insert("service_name".into(), json!(service_name));
            o.insert("transport".into(), Value::Object(t));
        }
        Transport::Httpupgrade { path, host } => {
            let mut t = Map::new();
            t.insert("type".into(), json!("httpupgrade"));
            t.insert("path".into(), json!(path));
            if !host.is_empty() {
                t.insert("host".into(), json!(host));
            }
            o.insert("transport".into(), Value::Object(t));
        }
    }

    Value::Object(o)
}

fn shadowsocks_outbound(s: &ProxyNode, ss: &ShadowsocksConfig, tag: &str) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), json!("shadowsocks"));
    o.insert("tag".into(), json!(tag));
    o.insert("server".into(), json!(s.server));
    o.insert("server_port".into(), json!(s.port));
    o.insert("method".into(), json!(ss.method));
    o.insert("password".into(), json!(ss.password));
    if let Some(p) = &ss.plugin {
        o.insert("plugin".into(), json!(p));
    }
    if let Some(po) = &ss.plugin_opts {
        o.insert("plugin_opts".into(), json!(po));
    }
    Value::Object(o)
}

fn inbounds(settings: &Settings) -> Value {
    match settings.mode {
        CoreMode::SystemProxy => json!([{
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": settings.mixed_port
        }]),
        CoreMode::Tun => json!([{
            "type": "tun",
            "tag": "tun-in",
            "interface_name": "lightgui-tun",
            "inet4_address": "172.19.0.1/30",
            "mtu": settings.tun_mtu,
            "auto_route": true,
            "strict_route": settings.tun_strict_route,
            "stack": settings.tun_stack,
            "endpoint_independent_nat": true,
            "udp_timeout": "5m"
        }]),
    }
}

fn route(settings: &Settings) -> Value {
    let mut rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
    ];

    let user_rules: Vec<Value> = settings
        .routing_rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|r| {
            let val = r.value.trim();
            if val.is_empty() {
                return None;
            }
            let mut map = Map::new();
            match r.rule_type {
                RuleType::Process => {
                    let matcher = r.process_matcher.unwrap_or_else(|| {
                        if val.contains('\\') || val.contains('/') {
                            ProcessMatcher::Path
                        } else {
                            ProcessMatcher::Name
                        }
                    });
                    match matcher {
                        ProcessMatcher::Name => {
                            map.insert("process_name".into(), json!([val]));
                        }
                        ProcessMatcher::Path => {
                            map.insert("process_path".into(), json!([val]));
                        }
                    }
                }
                RuleType::Domain => {
                    let matcher = r.domain_matcher.unwrap_or(DomainMatcher::Suffix);
                    match matcher {
                        DomainMatcher::Suffix => {
                            let clean = val.trim_start_matches('*').trim_start_matches('.');
                            if clean.is_empty() {
                                return None;
                            }
                            map.insert("domain_suffix".into(), json!([clean]));
                        }
                        DomainMatcher::Exact => {
                            map.insert("domain".into(), json!([val]));
                        }
                        DomainMatcher::Keyword => {
                            map.insert("domain_keyword".into(), json!([val]));
                        }
                        DomainMatcher::Regex => {
                            map.insert("domain_regex".into(), json!([val]));
                        }
                    }
                }
                RuleType::IpCidr => {
                    map.insert("ip_cidr".into(), json!([val]));
                }
            }

            match r.action {
                RuleAction::Proxy => {
                    map.insert("outbound".into(), json!("proxy"));
                }
                RuleAction::Direct => {
                    map.insert("outbound".into(), json!("direct"));
                }
                RuleAction::Block => {
                    map.insert("action".into(), json!("reject"));
                }
            }
            Some(Value::Object(map))
        })
        .collect();

    rules.extend(user_rules);

    if settings.discord_voice_direct {
        rules.push(json!({
            "process_name": ["Discord.exe", "DiscordCanary.exe", "DiscordPTB.exe"],
            "network": "udp",
            "outbound": "direct"
        }));
    }

    rules.extend([
        json!({ "ip_is_private": true, "outbound": "direct" }),
        json!({ "clash_mode": "Direct", "outbound": "direct" }),
        json!({ "clash_mode": "Global", "outbound": "proxy" }),
    ]);

    if settings.bypass_ru {
        rules.push(json!({ "rule_set": ["geosite-ru", "geoip-ru"], "outbound": "direct" }));
    }

    let mut route = Map::new();
    route.insert("rules".into(), Value::Array(rules));

    if settings.bypass_ru {
        route.insert(
            "rule_set".into(),
            json!([
                {
                    "tag": "geosite-ru",
                    "type": "remote",
                    "format": "binary",
                    "url": "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-rules-dat/release/geosite-category-ru.srs",
                    "download_detour": "direct",
                    "update_interval": "7d"
                },
                {
                    "tag": "geoip-ru",
                    "type": "remote",
                    "format": "binary",
                    "url": "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-rules-dat/release/geoip-ru.srs",
                    "download_detour": "direct",
                    "update_interval": "7d"
                }
            ]),
        );
    }

    let default_outbound = match settings.route_default {
        RouteTarget::Proxy => "proxy",
        RouteTarget::Direct => "direct",
    };
    route.insert("final".into(), json!(default_outbound));

    Value::Object(route)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ProxyNode, Security, Transport, VlessConfig};

    #[test]
    fn test_generate_config_structure() {
        let settings = Settings::default();
        let node = ProxyNode {
            id: "node-1".into(),
            name: "Server 1".into(),
            server: "1.2.3.4".into(),
            port: 443,
            last_ping_ms: None,
            favorite: false,
            total_up: 0,
            total_down: 0,
            raw: "".into(),
            kind: ProxyKind::Vless(VlessConfig {
                uuid: "uuid-123".into(),
                flow: "".into(),
                security: Security::None,
                sni: "".into(),
                fingerprint: "".into(),
                public_key: "".into(),
                short_id: "".into(),
                insecure: false,
                alpn: vec![],
                transport: Transport::Tcp,
            }),
        };

        let servers = vec![&node];
        let cfg = generate(&settings, &servers, Some("node-1"), 9090, "secret-token").unwrap();

        assert_eq!(cfg.clash_port, 9090);
        assert_eq!(cfg.clash_secret, "secret-token");

        let val = &cfg.json;
        assert!(val.get("inbounds").is_some());
        assert!(val.get("outbounds").is_some());
        assert!(val.get("route").is_some());
        assert!(val.get("dns").is_some());

        let inbounds = val["inbounds"].as_array().unwrap();
        assert_eq!(inbounds[0]["type"], "mixed");
        assert_eq!(inbounds[0]["listen_port"], settings.mixed_port);

        let outbounds = val["outbounds"].as_array().unwrap();
        assert_eq!(outbounds[0]["type"], "selector");
        assert_eq!(outbounds[0]["tag"], "proxy");
        assert_eq!(outbounds[1]["type"], "urltest");
        assert_eq!(outbounds[1]["tag"], "auto");
    }

    #[test]
    fn test_tun_inbound_config() {
        let mut settings = Settings::default();
        settings.mode = CoreMode::Tun;

        let cfg = generate(&settings, &[], None, 9090, "secret-token").unwrap();
        let inbounds = cfg.json["inbounds"].as_array().unwrap();
        assert_eq!(inbounds[0]["type"], "tun");
        assert_eq!(inbounds[0]["interface_name"], "lightgui-tun");
        assert_eq!(inbounds[0]["inet4_address"], "172.19.0.1/30");
    }
}
