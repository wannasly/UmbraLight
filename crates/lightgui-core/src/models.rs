use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Connection and Mode Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreMode {
    SystemProxy,
    Tun,
}

impl Default for CoreMode {
    fn default() -> Self {
        CoreMode::SystemProxy
    }
}

pub type Mode = CoreMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    Rule,
    GlobalProxy,
    DirectBypass,
}

impl Default for RoutingMode {
    fn default() -> Self {
        RoutingMode::Rule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteTarget {
    Proxy,
    Direct,
}

impl Default for RouteTarget {
    fn default() -> Self {
        RouteTarget::Proxy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Proxy,
    Direct,
    Block,
}

pub type AppRouteAction = RuleAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Process,
    Domain,
    IpCidr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessMatcher {
    Name,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainMatcher {
    Suffix,
    Exact,
    Keyword,
    Regex,
}

pub fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRule {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub rule_type: RuleType,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_matcher: Option<ProcessMatcher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_matcher: Option<DomainMatcher>,
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningProcess {
    pub pid: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionState {
    pub status: ConnStatus,
    pub server_id: Option<String>,
    pub server_name: Option<String>,
    pub mode: CoreMode,
    #[serde(default)]
    pub routing_mode: RoutingMode,
    pub since_ms: Option<i64>,
    pub error: Option<String>,
}

impl ConnectionState {
    pub fn disconnected(mode: CoreMode) -> Self {
        Self {
            status: ConnStatus::Disconnected,
            server_id: None,
            server_name: None,
            mode,
            routing_mode: RoutingMode::Rule,
            since_ms: None,
            error: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Server and Protocol Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Transport {
    Tcp,
    Ws {
        path: String,
        host: String,
    },
    Grpc {
        #[serde(rename = "serviceName")]
        service_name: String,
    },
    Httpupgrade {
        path: String,
        host: String,
    },
}

impl Default for Transport {
    fn default() -> Self {
        Transport::Tcp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    Reality,
    Tls,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Hysteria2Obfs {
    Salamander { password: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VlessConfig {
    pub uuid: String,
    #[serde(default)]
    pub flow: String,
    pub security: Security,
    #[serde(default)]
    pub sni: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub short_id: String,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub alpn: Vec<String>,
    #[serde(default)]
    pub transport: Transport,
}

pub type VlessNode = VlessConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hysteria2Config {
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfs: Option<Hysteria2Obfs>,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub sni: String,
    #[serde(default)]
    pub alpn: Vec<String>,
}

pub type Hysteria2Node = Hysteria2Config;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VMessConfig {
    pub uuid: String,
    #[serde(default)]
    pub alter_id: u32,
    #[serde(default = "default_vmess_security")]
    pub security: String,
    #[serde(default)]
    pub transport: Transport,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub sni: String,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub alpn: Vec<String>,
}

fn default_vmess_security() -> String {
    "auto".into()
}

pub type VMessNode = VMessConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrojanConfig {
    pub password: String,
    #[serde(default)]
    pub sni: String,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub alpn: Vec<String>,
    #[serde(default)]
    pub transport: Transport,
}

pub type TrojanNode = TrojanConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowsocksConfig {
    pub method: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_opts: Option<String>,
}

pub type ShadowsocksNode = ShadowsocksConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "protocol",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProxyKind {
    #[serde(rename = "vless")]
    Vless(VlessConfig),
    #[serde(rename = "hysteria2")]
    Hysteria2(Hysteria2Config),
    #[serde(rename = "vmess")]
    VMess(VMessConfig),
    #[serde(rename = "trojan")]
    Trojan(TrojanConfig),
    #[serde(rename = "shadowsocks")]
    Shadowsocks(ShadowsocksConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyNode {
    pub id: String,
    pub name: String,
    pub server: String,
    pub port: u16,

    #[serde(default)]
    pub last_ping_ms: Option<u32>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub total_up: u64,
    #[serde(default)]
    pub total_down: u64,
    #[serde(default)]
    pub raw: String,

    #[serde(flatten)]
    pub kind: ProxyKind,
}

pub type ServerEntry = ProxyNode;

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionQuota {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    /// unix seconds, 0 = never
    pub expire: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub quota: Option<SubscriptionQuota>,
    #[serde(default)]
    pub auto_update_hours: u32,
    #[serde(default)]
    pub support_url: Option<String>,
    #[serde(default)]
    pub web_page_url: Option<String>,
    #[serde(default)]
    pub panel_title: Option<String>,
    #[serde(default)]
    pub servers: Vec<ProxyNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStore {
    #[serde(default = "default_store_version")]
    pub version: u32,
    #[serde(default)]
    pub manual: Vec<ProxyNode>,
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
}

fn default_store_version() -> u32 {
    2
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self {
            version: default_store_version(),
            manual: Vec::new(),
            subscriptions: Vec::new(),
        }
    }
}

impl ProfileStore {
    pub fn all_servers(&self) -> impl Iterator<Item = &ServerEntry> {
        self.manual
            .iter()
            .chain(self.subscriptions.iter().flat_map(|s| s.servers.iter()))
    }

    pub fn find_server(&self, id: &str) -> Option<&ServerEntry> {
        self.all_servers().find(|s| s.id == id)
    }

    pub fn find_server_mut(&mut self, id: &str) -> Option<&mut ServerEntry> {
        self.manual
            .iter_mut()
            .chain(
                self.subscriptions
                    .iter_mut()
                    .flat_map(|s| s.servers.iter_mut()),
            )
            .find(|s| s.id == id)
    }
}

// ---------------------------------------------------------------------------
// Settings and Extras
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyBackup {
    pub enable: u32,
    #[serde(default)]
    pub server: Option<String>,
    #[serde(default, rename = "override")]
    pub bypass_list: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpStrategy {
    Ipv4Only,
    PreferIpv4,
    PreferIpv6,
    Ipv6Only,
}

impl Default for IpStrategy {
    fn default() -> Self {
        IpStrategy::Ipv4Only
    }
}

impl IpStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            IpStrategy::Ipv4Only => "ipv4_only",
            IpStrategy::PreferIpv4 => "prefer_ipv4",
            IpStrategy::PreferIpv6 => "prefer_ipv6",
            IpStrategy::Ipv6Only => "ipv6_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub language: String,
    pub accent: String,
    pub mode: CoreMode,
    pub routing_mode: RoutingMode,
    pub mixed_port: u16,
    pub selected_server_id: Option<String>,
    pub autostart: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub connect_on_startup: bool,
    pub log_level: String,
    pub bypass_ru: bool,
    pub route_default: RouteTarget,
    pub routing_rules: Vec<RouteRule>,
    pub tun_stack: String,
    pub tun_strict_route: bool,
    pub tun_mtu: u32,
    pub discord_voice_direct: bool,
    pub ip_strategy: IpStrategy,
    pub ping_url: String,
    pub reduce_motion: bool,
    pub server_sort: String,
    pub collapsed_groups: Vec<String>,
    pub github_mirror: String,
    pub sub_user_agent: String,
    pub send_hwid: bool,
    pub hwid: String,
    pub proxy_owned: bool,
    pub proxy_backup: ProxyBackup,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 2,
            language: "ru".into(),
            accent: "violet".into(),
            mode: CoreMode::SystemProxy,
            routing_mode: RoutingMode::Rule,
            mixed_port: 2080,
            selected_server_id: None,
            autostart: false,
            start_minimized: false,
            minimize_to_tray: true,
            connect_on_startup: false,
            log_level: "info".into(),
            bypass_ru: false,
            route_default: RouteTarget::Proxy,
            routing_rules: Vec::new(),
            tun_stack: "mixed".into(),
            tun_strict_route: true,
            tun_mtu: 9000,
            discord_voice_direct: true,
            ip_strategy: IpStrategy::Ipv4Only,
            ping_url: "https://www.gstatic.com/generate_204".into(),
            reduce_motion: false,
            server_sort: "default".into(),
            collapsed_groups: Vec::new(),
            github_mirror: String::new(),
            sub_user_agent: "v2rayN/7.13 lightgui/1.0".into(),
            send_hwid: true,
            hwid: String::new(),
            proxy_owned: false,
            proxy_backup: ProxyBackup::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub ts: i64,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficStats {
    pub up_bps: f64,
    pub down_bps: f64,
    pub up_total: f64,
    pub down_total: f64,
}
