use lightgui_core::models::*;
use lightgui_core::singbox::config::generate;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn find_singbox() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap();
    loop {
        let candidate = dir.join("resources").join("sing-box.exe");
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("sing-box.exe not found in resources/");
}

fn check_config(name: &str, settings: &Settings, servers: &[&ServerEntry], selected_id: Option<&str>) -> bool {
    let singbox_exe = find_singbox();
    println!("[TEST] Running check for: {}", name);
    let generated = match generate(settings, servers, selected_id, 9090, "secret-test") {
        Ok(g) => g,
        Err(e) => {
            println!("  FAIL: Config generation failed: {:?}", e);
            return false;
        }
    };

    let temp_dir = std::env::temp_dir().join(format!("singbox_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("config.json");
    fs::write(&config_path, serde_json::to_string_pretty(&generated.json).unwrap()).unwrap();

    let output = Command::new(&singbox_exe)
        .arg("check")
        .arg("-c")
        .arg(&config_path)
        .current_dir(&temp_dir)
        .output()
        .expect("failed to run sing-box check");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = fs::remove_dir_all(&temp_dir);

    if output.status.success() {
        println!("  PASS: sing-box check succeeded!");
        true
    } else {
        println!("  FAIL: sing-box check exited with code {:?}", output.status.code());
        if !stdout.trim().is_empty() {
            println!("  STDOUT:\n{}", stdout.trim());
        }
        if !stderr.trim().is_empty() {
            println!("  STDERR:\n{}", stderr.trim());
        }
        false
    }
}

fn main() {
    let singbox_exe = find_singbox();
    println!("Sing-box binary found at: {}", singbox_exe.display());

    let node = ProxyNode {
        id: "vless-1".into(),
        name: "VLESS Test Server".into(),
        server: "1.1.1.1".into(),
        port: 443,
        last_ping_ms: None,
        favorite: false,
        total_up: 0,
        total_down: 0,
        raw: "".into(),
        kind: ProxyKind::Vless(VlessConfig {
            uuid: "a0000000-0000-0000-0000-000000000001".into(),
            flow: "xtls-rprx-vision".into(),
            security: Security::Reality,
            sni: "gateway.icloud.com".into(),
            fingerprint: "chrome".into(),
            public_key: "jX_3n8-q5F4gV8hP-8e4Q8wK3vJ9a1m8z2k4n7p0q2s".into(),
            short_id: "01234567".into(),
            insecure: false,
            alpn: vec!["h2".into(), "http/1.1".into()],
            transport: Transport::Tcp,
        }),
    };
    let servers = vec![&node];

    let mut all_pass = true;

    // Test 1: Default SystemProxy mode
    let s1 = Settings::default();
    if !check_config("Default SystemProxy mode", &s1, &servers, Some("vless-1")) {
        all_pass = false;
    }

    // Test 2: TUN mode
    let mut s2 = Settings::default();
    s2.mode = CoreMode::Tun;
    if !check_config("TUN mode", &s2, &servers, Some("vless-1")) {
        all_pass = false;
    }

    // Test 3: Routing rules with Bypass RU
    let mut s3 = Settings::default();
    s3.bypass_ru = true;
    s3.routing_rules = vec![
        RouteRule {
            id: "r1".into(),
            enabled: true,
            rule_type: RuleType::Domain,
            value: "google.com".into(),
            process_matcher: None,
            domain_matcher: Some(DomainMatcher::Suffix),
            action: RuleAction::Proxy,
            description: None,
        },
    ];
    if !check_config("Routing rules & Bypass RU", &s3, &servers, Some("vless-1")) {
        all_pass = false;
    }

    if all_pass {
        println!("\nALL CONFIG CHECKS PASSED!");
    } else {
        println!("\nCONFIG CHECKS FAILED!");
        std::process::exit(1);
    }
}
