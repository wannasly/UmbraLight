use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::Mutex as TokioMutex;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

use lightgui_core::error::{Error, Result};
use lightgui_core::hwid::{device_model, device_os, hwid, os_version};
use lightgui_core::ipc::protocol::{IpcRequest, IpcResponse, StatusInfo, DEFAULT_PIPE_NAME};
use lightgui_core::models::{
    ConnStatus, ConnectionState, CoreMode, ProfileStore, RoutingMode, ServerEntry, Settings,
};
use lightgui_core::net::processes::list_running_processes;
use lightgui_core::net::system_proxy::{apply_proxy, read_current, restore_proxy};
use lightgui_core::singbox::clash_api;
use lightgui_core::singbox::config::generate as generate_singbox_config;
use lightgui_core::singbox::process::{
    check_config, find_singbox_binary, now_ms, CoreProcess, LogBuffer,
};
use lightgui_core::storage::{
    default_data_dir, load_profiles, load_settings, save_profiles, save_settings,
};
use lightgui_core::subscription::{fetch_subscription, merge_servers, DeviceIdentity};

pub const WM_TRAY_UPDATE: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_USER + 101;

pub struct DaemonNotifier {
    hwnd: Arc<AtomicPtr<std::ffi::c_void>>,
}

impl DaemonNotifier {
    pub fn new() -> Self {
        Self {
            hwnd: Arc::new(AtomicPtr::new(std::ptr::null_mut())),
        }
    }

    pub fn set_hwnd(&self, hwnd: HWND) {
        self.hwnd.store(hwnd, Ordering::SeqCst);
    }

    pub fn notify(&self) {
        let h = self.hwnd.load(Ordering::SeqCst);
        if !h.is_null() {
            unsafe {
                PostMessageW(h, WM_TRAY_UPDATE, 0, 0);
            }
        }
    }
}

pub struct DaemonSyncState {
    pub conn_state: ConnectionState,
    pub settings: Settings,
    pub profiles: ProfileStore,
    pub data_dir: PathBuf,
    pub clash_port: u16,
    pub clash_secret: String,
    pub log_buffer: Arc<LogBuffer>,
    pub tag_by_server_id: HashMap<String, String>,
}

pub struct Daemon {
    pub sync_state: Arc<RwLock<DaemonSyncState>>,
    pub runner: Arc<TokioMutex<Option<CoreProcess>>>,
    pub notifier: Arc<DaemonNotifier>,
}

impl Daemon {
    pub fn init(custom_data_dir: Option<PathBuf>) -> Arc<Self> {
        let data_dir = custom_data_dir.unwrap_or_else(default_data_dir);
        let mut settings = load_settings(&data_dir);
        let profiles = load_profiles(&data_dir).unwrap_or_default();
        let log_buffer = Arc::new(LogBuffer::new());

        // Startup recovery: restore proxy if an earlier session crashed while proxy was owned
        if settings.proxy_owned {
            log_buffer.push_now("warn", "recovering previous unclosed system proxy setting");
            match restore_proxy(&settings.proxy_backup) {
                Ok(()) => {
                    settings.proxy_owned = false;
                    let _ = save_settings(&data_dir, &settings);
                }
                Err(e) => {
                    log_buffer.push_now("error", format!("could not recover system proxy: {e}"))
                }
            }
        }

        let clash_port = 9090;
        let clash_secret = "lightgui_local_secret".to_string();

        let conn_state = ConnectionState {
            status: ConnStatus::Disconnected,
            server_id: settings.selected_server_id.clone(),
            server_name: settings
                .selected_server_id
                .as_deref()
                .and_then(|id| profiles.find_server(id).map(|s| s.name.clone())),
            mode: settings.mode,
            routing_mode: settings.routing_mode,
            since_ms: None,
            error: None,
        };

        let sync_state = Arc::new(RwLock::new(DaemonSyncState {
            conn_state,
            settings,
            profiles,
            data_dir,
            clash_port,
            clash_secret,
            log_buffer,
            tag_by_server_id: HashMap::new(),
        }));

        let daemon = Arc::new(Self {
            sync_state,
            runner: Arc::new(TokioMutex::new(None)),
            notifier: Arc::new(DaemonNotifier::new()),
        });

        // Check connect_on_startup
        let auto_connect = daemon.sync_state.read().settings.connect_on_startup;
        if auto_connect {
            let d = daemon.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = d.connect(None).await;
            });
        }

        daemon
    }

    pub fn get_sync_state(&self) -> (ConnectionState, ProfileStore, Settings) {
        let g = self.sync_state.read();
        (g.conn_state.clone(), g.profiles.clone(), g.settings.clone())
    }

    pub async fn update_settings(&self, settings: Settings) -> Result<()> {
        let (dir, reconnect, selected) = {
            let g = self.sync_state.read();
            let old = &g.settings;
            let reconnect = g.conn_state.status == ConnStatus::Connected
                && (old.mode != settings.mode
                    || old.routing_mode != settings.routing_mode
                    || old.mixed_port != settings.mixed_port
                    || old.proxy_bypass_list != settings.proxy_bypass_list
                    || old.log_level != settings.log_level
                    || old.bypass_ru != settings.bypass_ru
                    || old.route_default != settings.route_default
                    || old.routing_rules != settings.routing_rules
                    || old.tun_stack != settings.tun_stack
                    || old.tun_strict_route != settings.tun_strict_route
                    || old.tun_mtu != settings.tun_mtu
                    || old.discord_voice_direct != settings.discord_voice_direct
                    || old.ip_strategy != settings.ip_strategy
                    || old.ping_url != settings.ping_url);
            (
                g.data_dir.clone(),
                reconnect,
                g.conn_state.server_id.clone(),
            )
        };
        save_settings(&dir, &settings)?;
        {
            let mut g = self.sync_state.write();
            g.conn_state.mode = settings.mode;
            g.conn_state.routing_mode = settings.routing_mode;
            g.settings = settings;
        }
        self.notifier.notify();
        if reconnect {
            self.disconnect().await?;
            self.connect(selected).await?;
        }
        Ok(())
    }

    pub fn update_profiles(&self, profiles: ProfileStore) -> Result<()> {
        let mut g = self.sync_state.write();
        save_profiles(&g.data_dir, &profiles)?;
        g.profiles = profiles;
        self.notifier.notify();
        Ok(())
    }

    pub async fn connect(&self, target_server_id: Option<String>) -> Result<()> {
        let (data_dir, mut settings, profiles, clash_port, clash_secret, log_buffer) = {
            let mut g = self.sync_state.write();
            g.conn_state.status = ConnStatus::Connecting;
            g.conn_state.error = None;
            if let Some(ref id) = target_server_id {
                g.conn_state.server_id = Some(id.clone());
                g.settings.selected_server_id = Some(id.clone());
            }
            (
                g.data_dir.clone(),
                g.settings.clone(),
                g.profiles.clone(),
                g.clash_port,
                g.clash_secret.clone(),
                g.log_buffer.clone(),
            )
        };
        self.notifier.notify();

        let res = async {
            let exe = find_singbox_binary(Some(&data_dir)).ok_or(Error::CoreNotInstalled)?;

            let servers: Vec<&ServerEntry> = profiles.all_servers().collect();
            if servers.is_empty() {
                return Err(Error::Internal(
                    "No proxy servers available to connect".into(),
                ));
            }

            let sel_id = target_server_id
                .or_else(|| settings.selected_server_id.clone())
                .or_else(|| servers.first().map(|s| s.id.clone()));

            let gen = generate_singbox_config(
                &settings,
                &servers,
                sel_id.as_deref(),
                clash_port,
                &clash_secret,
            )?;

            // Store tag map
            {
                let mut g = self.sync_state.write();
                g.tag_by_server_id = gen.tag_by_server_id.clone();
            }

            // Write config
            let config_path = data_dir.join("config.json");
            let json_bytes = serde_json::to_vec_pretty(&gen.json)?;
            fs::write(&config_path, &json_bytes)?;

            check_config(&exe, &config_path, &data_dir).await?;

            // Stop existing process if any
            let mut r_guard = self.runner.lock().await;
            if let Some(mut old_proc) = r_guard.take() {
                let _ = old_proc.stop().await;
            }

            // Spawn core
            log_buffer.push_now(
                "info",
                format!("starting sing-box with config: {}", config_path.display()),
            );
            let mut proc = CoreProcess::spawn(&exe, &config_path, &data_dir, log_buffer.clone())?;
            proc.confirm_ready(clash_port, &clash_secret).await?;
            let process_id = proc.id();
            *r_guard = Some(proc);

            // Configure System Proxy if in SystemProxy mode
            if settings.mode == CoreMode::SystemProxy {
                if !settings.proxy_owned {
                    settings.proxy_backup = read_current()?;
                    settings.proxy_owned = true;
                    save_settings(&data_dir, &settings)?;
                    self.sync_state.write().settings = settings.clone();
                }
                apply_proxy(settings.mixed_port, &settings.proxy_bypass_list)?;
            }

            let active_name = sel_id
                .as_deref()
                .and_then(|id| profiles.find_server(id).map(|s| s.name.clone()))
                .unwrap_or_else(|| "Auto".into());

            // Update state
            {
                let mut g = self.sync_state.write();
                g.conn_state.status = ConnStatus::Connected;
                g.conn_state.server_id = sel_id.clone();
                g.conn_state.server_name = Some(active_name);
                g.conn_state.since_ms = Some(now_ms());
                g.conn_state.error = None;
                g.settings = settings.clone();
                let _ = save_settings(&data_dir, &settings);
            }

            log_buffer.push_now("info", "core started and connected successfully");
            self.watch_core(process_id);
            Ok::<(), Error>(())
        }
        .await;

        if let Err(ref e) = res {
            if let Some(mut proc) = self.runner.lock().await.take() {
                let _ = proc.stop().await;
            }
            let mut g = self.sync_state.write();
            if g.settings.proxy_owned {
                if let Err(restore_error) = restore_proxy(&g.settings.proxy_backup) {
                    g.log_buffer.push_now(
                        "error",
                        format!("could not restore system proxy: {restore_error}"),
                    );
                } else {
                    g.settings.proxy_owned = false;
                    let _ = save_settings(&g.data_dir, &g.settings);
                }
            }
            g.conn_state.status = ConnStatus::Error;
            g.conn_state.error = Some(e.to_string());
            g.log_buffer
                .push_now("error", format!("connection failed: {e}"));
        }
        self.notifier.notify();
        res
    }

    pub async fn disconnect(&self) -> Result<()> {
        let (data_dir, mut settings, log_buffer) = {
            let mut g = self.sync_state.write();
            g.conn_state.status = ConnStatus::Disconnecting;
            (g.data_dir.clone(), g.settings.clone(), g.log_buffer.clone())
        };
        self.notifier.notify();

        // 1. Restore system proxy if owned
        let mut restore_error = None;
        if settings.proxy_owned {
            match restore_proxy(&settings.proxy_backup) {
                Ok(()) => {
                    settings.proxy_owned = false;
                    let _ = save_settings(&data_dir, &settings);
                }
                Err(e) => {
                    log_buffer.push_now("error", format!("could not restore system proxy: {e}"));
                    restore_error = Some(e);
                }
            }
        }

        // 2. Stop child process
        let mut r_guard = self.runner.lock().await;
        if let Some(mut proc) = r_guard.take() {
            let _ = proc.stop().await;
        }

        // 3. Update state
        {
            let mut g = self.sync_state.write();
            g.conn_state.status = if restore_error.is_some() {
                ConnStatus::Error
            } else {
                ConnStatus::Disconnected
            };
            g.conn_state.since_ms = None;
            g.conn_state.error = restore_error.as_ref().map(ToString::to_string);
            g.settings = settings;
        }

        log_buffer.push_now("info", "core disconnected");
        self.notifier.notify();
        if let Some(e) = restore_error {
            return Err(e);
        }
        Ok(())
    }

    fn watch_core(&self, process_id: Option<u32>) {
        let runner = self.runner.clone();
        let state = self.sync_state.clone();
        let notifier = self.notifier.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                if state.read().conn_state.status != ConnStatus::Connected {
                    break;
                }
                let mut guard = runner.lock().await;
                let Some(proc) = guard.as_mut() else {
                    break;
                };
                if proc.id() != process_id {
                    break;
                }
                let exit = proc.try_wait();
                if matches!(exit, Ok(None)) {
                    continue;
                }
                guard.take();
                let mut g = state.write();
                if g.conn_state.status == ConnStatus::Connected {
                    if g.settings.proxy_owned {
                        if let Err(e) = restore_proxy(&g.settings.proxy_backup) {
                            g.log_buffer.push_now(
                                "error",
                                format!("could not restore system proxy after crash: {e}"),
                            );
                        } else {
                            g.settings.proxy_owned = false;
                            let _ = save_settings(&g.data_dir, &g.settings);
                        }
                    }
                    g.conn_state.status = ConnStatus::Error;
                    g.conn_state.since_ms = None;
                    let msg = match exit {
                        Ok(Some(status)) => format!("sing-box exited unexpectedly: {status}"),
                        Err(e) => format!("sing-box process failed: {e}"),
                        Ok(None) => unreachable!(),
                    };
                    g.conn_state.error = Some(msg.clone());
                    g.log_buffer.push_now("error", msg);
                    notifier.notify();
                }
                break;
            }
        });
    }

    pub async fn switch_server(&self, server_id: &str) -> Result<()> {
        let (is_connected, clash_port, clash_secret, tag) = {
            let g = self.sync_state.read();
            let connected = g.conn_state.status == ConnStatus::Connected;
            let tag = if server_id == "auto" {
                Some("auto".to_string())
            } else {
                g.tag_by_server_id.get(server_id).cloned()
            };
            (connected, g.clash_port, g.clash_secret.clone(), tag)
        };

        if is_connected {
            if let Some(t) = tag {
                match clash_api::switch_server(clash_port, &clash_secret, &t).await {
                    Ok(()) => {
                        let mut g = self.sync_state.write();
                        g.conn_state.server_id = Some(server_id.to_string());
                        g.conn_state.server_name = if server_id == "auto" {
                            Some("Автовыбор".into())
                        } else {
                            g.profiles.find_server(server_id).map(|s| s.name.clone())
                        };
                        g.settings.selected_server_id = Some(server_id.to_string());
                        let _ = save_settings(&g.data_dir, &g.settings);
                        self.notifier.notify();
                        return Ok(());
                    }
                    Err(e) => {
                        self.sync_state.read().log_buffer.push_now(
                            "warn",
                            format!("clash api switch failed ({e}), restarting core"),
                        );
                    }
                }
            }
            // Fallback: reconnect with new server
            self.connect(Some(server_id.to_string())).await
        } else {
            let mut g = self.sync_state.write();
            g.settings.selected_server_id = Some(server_id.to_string());
            g.conn_state.server_id = Some(server_id.to_string());
            g.conn_state.server_name = if server_id == "auto" {
                Some("Автовыбор".into())
            } else {
                g.profiles.find_server(server_id).map(|s| s.name.clone())
            };
            let _ = save_settings(&g.data_dir, &g.settings);
            self.notifier.notify();
            Ok(())
        }
    }

    pub async fn set_routing_mode(&self, mode: RoutingMode) -> Result<()> {
        let is_connected = {
            let mut g = self.sync_state.write();
            g.settings.routing_mode = mode;
            g.conn_state.routing_mode = mode;
            let _ = save_settings(&g.data_dir, &g.settings);
            g.conn_state.status == ConnStatus::Connected
        };
        self.notifier.notify();

        if is_connected {
            let cur_id = self.sync_state.read().conn_state.server_id.clone();
            self.connect(cur_id).await?;
        }
        Ok(())
    }

    pub async fn set_core_mode(&self, mode: CoreMode) -> Result<()> {
        let (is_connected, cur_id) = {
            let mut g = self.sync_state.write();
            let conn = g.conn_state.status == ConnStatus::Connected;
            g.settings.mode = mode;
            g.conn_state.mode = mode;
            let _ = save_settings(&g.data_dir, &g.settings);
            (conn, g.conn_state.server_id.clone())
        };
        self.notifier.notify();

        if is_connected {
            self.disconnect().await?;
            self.connect(cur_id).await?;
        }
        Ok(())
    }

    pub async fn refresh_subscriptions(&self, sub_id: Option<String>) -> Result<()> {
        let (data_dir, mut profiles, settings_clone) = {
            let g = self.sync_state.read();
            (g.data_dir.clone(), g.profiles.clone(), g.settings.clone())
        };

        let identity = DeviceIdentity {
            hwid: hwid(Some(&settings_clone.hwid)),
            os: device_os(),
            os_version: os_version(),
            model: device_model(),
        };

        let mut updated = false;
        let mut first_error = None;
        for sub in &mut profiles.subscriptions {
            if let Some(ref target) = sub_id {
                if &sub.id != target {
                    continue;
                }
            }
            let device = settings_clone.send_hwid.then_some(&identity);
            match fetch_subscription(&sub.url, Some(&settings_clone.sub_user_agent), device).await {
                Ok(fetched) => {
                    if fetched.servers.is_empty() {
                        let detail = fetched
                            .errors
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "no supported servers".into());
                        if first_error.is_none() {
                            first_error = Some(format!("{}: {detail}", sub.name));
                        }
                        continue;
                    }
                    sub.servers = merge_servers(&sub.servers, fetched.servers);
                    sub.updated_at = Some(chrono_now_iso());
                    sub.quota = fetched.quota;
                    if let Some(title) = fetched.title {
                        sub.panel_title = Some(title);
                    }
                    if let Some(hours) = fetched.update_interval_hours {
                        sub.auto_update_hours = hours;
                    }
                    if let Some(url) = fetched.support_url {
                        sub.support_url = Some(url);
                    }
                    if let Some(url) = fetched.web_page_url {
                        sub.web_page_url = Some(url);
                    }
                    updated = true;
                }
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(e.to_string());
                    }
                    self.sync_state.read().log_buffer.push_now(
                        "error",
                        format!("failed to refresh subscription {}: {e}", sub.name),
                    );
                }
            }
        }

        if updated {
            save_profiles(&data_dir, &profiles)?;
            let mut g = self.sync_state.write();
            g.profiles = profiles;
        }

        self.notifier.notify();
        if let Some(error) = first_error {
            return Err(Error::Network(error));
        }
        Ok(())
    }

    pub async fn ping_server(&self, server_id: &str) -> Option<u32> {
        let (server_info, is_connected, clash_port, clash_secret, ping_url, tag) = {
            let g = self.sync_state.read();
            let s = g.profiles.find_server(server_id).cloned();
            let connected = g.conn_state.status == ConnStatus::Connected;
            let tag = g.tag_by_server_id.get(server_id).cloned();
            (
                s,
                connected,
                g.clash_port,
                g.clash_secret.clone(),
                g.settings.ping_url.clone(),
                tag,
            )
        };

        let node = server_info?;

        // 1. Try clash delay if connected
        if is_connected {
            if let Some(t) = tag {
                if let Ok(delay) =
                    clash_api::test_delay(clash_port, &clash_secret, &t, &ping_url, 3000).await
                {
                    self.record_server_ping(server_id, delay);
                    return Some(delay);
                }
            }
        }

        // 2. Fallback to TCP ping
        let delay = lightgui_core::net::ping::tcp_ping(&node.server, node.port).await;
        if let Some(d) = delay {
            self.record_server_ping(server_id, d);
        }
        delay
    }

    fn record_server_ping(&self, server_id: &str, delay_ms: u32) {
        let mut g = self.sync_state.write();
        if let Some(s) = g.profiles.find_server_mut(server_id) {
            s.last_ping_ms = Some(delay_ms);
        }
        let _ = save_profiles(&g.data_dir, &g.profiles);
    }

    pub async fn restart_core(&self) -> Result<()> {
        let cur_id = self.sync_state.read().conn_state.server_id.clone();
        self.disconnect().await?;
        self.connect(cur_id).await
    }

    pub async fn shutdown(&self) {
        let _ = self.disconnect().await;
    }

    pub async fn handle_ipc_request(&self, request: IpcRequest) -> IpcResponse {
        match request {
            IpcRequest::GetStatus => {
                let g = self.sync_state.read();
                let uptime = if g.conn_state.status == ConnStatus::Connected {
                    g.conn_state
                        .since_ms
                        .map(|s| (now_ms() - s).max(0) as u64 / 1000)
                } else {
                    None
                };
                IpcResponse::Status(StatusInfo {
                    state: g.conn_state.clone(),
                    active_server_name: g.conn_state.server_name.clone(),
                    uptime_secs: uptime,
                })
            }
            IpcRequest::Connect { server_id } => match self.connect(server_id).await {
                Ok(()) => IpcResponse::Success,
                Err(e) => IpcResponse::Error(e.to_string()),
            },
            IpcRequest::Disconnect => match self.disconnect().await {
                Ok(()) => IpcResponse::Success,
                Err(e) => IpcResponse::Error(e.to_string()),
            },
            IpcRequest::SwitchServer { server_id } => match self.switch_server(&server_id).await {
                Ok(()) => IpcResponse::Success,
                Err(e) => IpcResponse::Error(e.to_string()),
            },
            IpcRequest::SetRoutingMode { mode } => match self.set_routing_mode(mode).await {
                Ok(()) => IpcResponse::Success,
                Err(e) => IpcResponse::Error(e.to_string()),
            },
            IpcRequest::SaveSettings { settings } => match self.update_settings(settings).await {
                Ok(()) => IpcResponse::Success,
                Err(e) => IpcResponse::Error(e.to_string()),
            },
            IpcRequest::SaveProfiles { profiles } => match self.update_profiles(profiles) {
                Ok(()) => IpcResponse::Success,
                Err(e) => IpcResponse::Error(e.to_string()),
            },
            IpcRequest::RefreshSubscription { sub_id } => {
                match self.refresh_subscriptions(sub_id).await {
                    Ok(()) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error(e.to_string()),
                }
            }
            IpcRequest::GetLogs => {
                let logs = self.sync_state.read().log_buffer.tail(100);
                IpcResponse::Logs(logs)
            }
            IpcRequest::GetRunningProcesses => {
                let procs = list_running_processes();
                IpcResponse::Processes(procs)
            }
            IpcRequest::PingServer { server_id } => {
                let delay = self.ping_server(&server_id).await;
                IpcResponse::PingResult { delay_ms: delay }
            }
        }
    }
}

fn chrono_now_iso() -> String {
    let now = std::time::SystemTime::now();
    let dur = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}

/// Runs the IPC Named Pipe server loop on `\\.\pipe\lightgui_ipc`.
pub async fn run_ipc_server(daemon: Arc<Daemon>) {
    let mut first_instance = true;
    loop {
        match lightgui_core::ipc::pipe::create_server(DEFAULT_PIPE_NAME, first_instance) {
            Ok(server) => {
                first_instance = false;
                match server.connect().await {
                    Ok(()) => {
                        let d = daemon.clone();
                        tokio::spawn(async move {
                            handle_ipc_client(server, d).await;
                        });
                    }
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

async fn handle_ipc_client(
    mut pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    daemon: Arc<Daemon>,
) {
    use lightgui_core::ipc::pipe::{read_frame, write_frame};

    loop {
        match read_frame::<_, IpcRequest>(&mut pipe).await {
            Ok(req) => {
                let resp = daemon.handle_ipc_request(req).await;
                if write_frame(&mut pipe, &resp).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_init() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("lightgui-daemon-test-{ts}"));
        let daemon = Daemon::init(Some(temp_dir.clone()));
        let (state, _, _) = daemon.get_sync_state();
        assert_eq!(state.status, ConnStatus::Disconnected);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_daemon_ipc_handling() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("lightgui-daemon-ipc-test-{ts}"));
        let daemon = Daemon::init(Some(temp_dir.clone()));

        // 1. GetStatus
        let status_resp = daemon.handle_ipc_request(IpcRequest::GetStatus).await;
        match status_resp {
            IpcResponse::Status(info) => {
                assert_eq!(info.state.status, ConnStatus::Disconnected);
            }
            other => panic!("Expected Status response, got {other:?}"),
        }

        // 2. SetRoutingMode
        let mode_resp = daemon
            .handle_ipc_request(IpcRequest::SetRoutingMode {
                mode: RoutingMode::GlobalProxy,
            })
            .await;
        assert_eq!(mode_resp, IpcResponse::Success);
        assert_eq!(
            daemon.sync_state.read().settings.routing_mode,
            RoutingMode::GlobalProxy
        );

        let mut settings = daemon.sync_state.read().settings.clone();
        settings.mixed_port = 28880;
        assert_eq!(
            daemon
                .handle_ipc_request(IpcRequest::SaveSettings { settings })
                .await,
            IpcResponse::Success
        );
        assert_eq!(daemon.sync_state.read().settings.mixed_port, 28880);

        let mut profiles = ProfileStore::default();
        profiles.version = 3;
        assert_eq!(
            daemon
                .handle_ipc_request(IpcRequest::SaveProfiles { profiles })
                .await,
            IpcResponse::Success
        );
        assert_eq!(daemon.sync_state.read().profiles.version, 3);

        // 3. GetLogs
        daemon
            .sync_state
            .read()
            .log_buffer
            .push_now("info", "test log entry");
        let logs_resp = daemon.handle_ipc_request(IpcRequest::GetLogs).await;
        match logs_resp {
            IpcResponse::Logs(lines) => {
                assert!(lines.iter().any(|l| l.message == "test log entry"));
            }
            other => panic!("Expected Logs response, got {other:?}"),
        }

        // 4. GetRunningProcesses
        let proc_resp = daemon
            .handle_ipc_request(IpcRequest::GetRunningProcesses)
            .await;
        match proc_resp {
            IpcResponse::Processes(_) => {}
            other => panic!("Expected Processes response, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn refresh_subscription_preserves_servers_on_repeat_and_bad_response() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            for index in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 2048];
                let _ = socket.read(&mut request).await.unwrap();
                let body = if index == 2 {
                    "[]"
                } else {
                    "vless://11111111-2222-3333-4444-555555555555@example.com:443?security=none#Server"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lightgui-refresh-test-{nonce}"));
        let daemon = Daemon::init(Some(dir.clone()));
        let profiles: ProfileStore = serde_json::from_value(serde_json::json!({
            "version": 2, "manual": [], "subscriptions": [{"id": "sub", "name": "Test", "url": format!("http://127.0.0.1:{port}/sub"), "servers": []}]
        })).unwrap();
        daemon.update_profiles(profiles).unwrap();
        daemon
            .refresh_subscriptions(Some("sub".into()))
            .await
            .unwrap();
        let id = daemon.sync_state.read().profiles.subscriptions[0].servers[0]
            .id
            .clone();
        daemon
            .refresh_subscriptions(Some("sub".into()))
            .await
            .unwrap();
        assert_eq!(
            daemon.sync_state.read().profiles.subscriptions[0].servers[0].id,
            id
        );
        assert!(daemon
            .refresh_subscriptions(Some("sub".into()))
            .await
            .is_err());
        assert_eq!(
            daemon.sync_state.read().profiles.subscriptions[0].servers[0].id,
            id
        );
        server.await.unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
