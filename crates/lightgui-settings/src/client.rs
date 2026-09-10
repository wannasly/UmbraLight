use std::path::{Path, PathBuf};
use lightgui_core::error::{Error, Result};
use lightgui_core::ipc::protocol::{IpcRequest, IpcResponse, StatusInfo, DEFAULT_PIPE_NAME};
use lightgui_core::models::{ConnectionState, LogLine, ProfileStore, RoutingMode, RunningProcess, Settings};
use lightgui_core::storage::{default_data_dir, load_profiles, load_settings, save_profiles, save_settings};

pub struct IpcClient {
    pipe_name: String,
    data_dir: PathBuf,
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            pipe_name: DEFAULT_PIPE_NAME.to_string(),
            data_dir: default_data_dir(),
        }
    }

    pub fn with_pipe_and_dir(pipe_name: impl Into<String>, data_dir: PathBuf) -> Self {
        Self {
            pipe_name: pipe_name.into(),
            data_dir,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn load_settings(&self) -> Settings {
        load_settings(&self.data_dir)
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        save_settings(&self.data_dir, settings)
    }

    pub fn load_profiles(&self) -> Result<ProfileStore> {
        load_profiles(&self.data_dir)
    }

    pub fn save_profiles(&self, profiles: &ProfileStore) -> Result<()> {
        save_profiles(&self.data_dir, profiles)
    }

    fn send_ipc(&self, req: &IpcRequest) -> Result<IpcResponse> {
        use std::io::{Read, Write};
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.pipe_name)?;

        let bytes = serde_json::to_vec(req)?;
        let len = (bytes.len() as u32).to_le_bytes();
        file.write_all(&len)?;
        file.write_all(&bytes)?;
        file.flush()?;

        let mut len_buf = [0u8; 4];
        file.read_exact(&mut len_buf)?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        if resp_len > 10 * 1024 * 1024 {
            return Err(Error::Parse("IPC frame exceeded maximum size".into()));
        }
        let mut resp_buf = vec![0u8; resp_len];
        file.read_exact(&mut resp_buf)?;
        let resp: IpcResponse = serde_json::from_slice(&resp_buf)?;
        Ok(resp)
    }

    pub fn get_status(&self) -> Result<StatusInfo> {
        if let Ok(IpcResponse::Status(info)) = self.send_ipc(&IpcRequest::GetStatus) {
            return Ok(info);
        }

        // Fallback: direct local storage
        let settings = self.load_settings();
        let profiles = self.load_profiles().unwrap_or_default();
        let active_server_name = settings.selected_server_id.as_deref().and_then(|id| {
            profiles.find_server(id).map(|s| s.name.clone())
        });

        Ok(StatusInfo {
            state: ConnectionState::disconnected(settings.mode),
            active_server_name,
            uptime_secs: None,
        })
    }

    pub fn connect(&self, server_id: Option<String>) -> Result<()> {
        if let Ok(resp) = self.send_ipc(&IpcRequest::Connect {
            server_id: server_id.clone(),
        }) {
            match resp {
                IpcResponse::Success => return Ok(()),
                IpcResponse::Error(msg) => return Err(Error::Internal(msg)),
                _ => {}
            }
        }

        // Fallback: update selected server in settings
        if let Some(id) = server_id {
            let mut settings = self.load_settings();
            settings.selected_server_id = Some(id);
            self.save_settings(&settings)?;
        }
        Ok(())
    }

    pub fn disconnect(&self) -> Result<()> {
        if let Ok(resp) = self.send_ipc(&IpcRequest::Disconnect) {
            match resp {
                IpcResponse::Success => return Ok(()),
                IpcResponse::Error(msg) => return Err(Error::Internal(msg)),
                _ => {}
            }
        }
        Ok(())
    }

    pub fn switch_server(&self, server_id: &str) -> Result<()> {
        if let Ok(resp) = self.send_ipc(&IpcRequest::SwitchServer {
            server_id: server_id.to_string(),
        }) {
            match resp {
                IpcResponse::Success => return Ok(()),
                IpcResponse::Error(msg) => return Err(Error::Internal(msg)),
                _ => {}
            }
        }

        // Fallback: update selected server in settings
        let mut settings = self.load_settings();
        settings.selected_server_id = Some(server_id.to_string());
        self.save_settings(&settings)?;
        Ok(())
    }

    pub fn set_routing_mode(&self, mode: RoutingMode) -> Result<()> {
        if let Ok(resp) = self.send_ipc(&IpcRequest::SetRoutingMode { mode }) {
            match resp {
                IpcResponse::Success => return Ok(()),
                IpcResponse::Error(msg) => return Err(Error::Internal(msg)),
                _ => {}
            }
        }

        // Fallback: update routing mode in settings
        let mut settings = self.load_settings();
        settings.routing_mode = mode;
        self.save_settings(&settings)?;
        Ok(())
    }

    pub fn refresh_subscription(&self, sub_id: Option<String>) -> Result<()> {
        if let Ok(resp) = self.send_ipc(&IpcRequest::RefreshSubscription { sub_id }) {
            match resp {
                IpcResponse::Success => return Ok(()),
                IpcResponse::Error(msg) => return Err(Error::Internal(msg)),
                _ => {}
            }
        }

        // Fallback: touches profiles
        let profiles = self.load_profiles().unwrap_or_default();
        let _ = self.save_profiles(&profiles);
        Ok(())
    }

    pub fn get_logs(&self) -> Result<Vec<LogLine>> {
        if let Ok(IpcResponse::Logs(logs)) = self.send_ipc(&IpcRequest::GetLogs) {
            return Ok(logs);
        }
        Ok(Vec::new())
    }

    pub fn get_processes(&self) -> Result<Vec<RunningProcess>> {
        if let Ok(IpcResponse::Processes(procs)) = self.send_ipc(&IpcRequest::GetRunningProcesses) {
            return Ok(procs);
        }
        Ok(lightgui_core::net::processes::list_running_processes())
    }

    pub fn ping_server(&self, server_id: &str) -> Result<Option<u32>> {
        if let Ok(IpcResponse::PingResult { delay_ms }) = self.send_ipc(&IpcRequest::PingServer {
            server_id: server_id.to_string(),
        }) {
            return Ok(delay_ms);
        }

        // Fallback: direct TCP connect
        let profiles = self.load_profiles().unwrap_or_default();
        if let Some(srv) = profiles.find_server(server_id) {
            use std::net::ToSocketAddrs;
            if let Ok(mut addrs) = (srv.server.as_str(), srv.port).to_socket_addrs() {
                if let Some(addr) = addrs.next() {
                    let started = std::time::Instant::now();
                    if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2)).is_ok() {
                        let ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
                        return Ok(Some(ms));
                    }
                }
            }
        }
        Ok(None)
    }
}

// Module-level free functions
pub fn get_status() -> Result<StatusInfo> {
    IpcClient::new().get_status()
}

pub fn connect(server_id: Option<String>) -> Result<()> {
    IpcClient::new().connect(server_id)
}

pub fn disconnect() -> Result<()> {
    IpcClient::new().disconnect()
}

pub fn switch_server(server_id: &str) -> Result<()> {
    IpcClient::new().switch_server(server_id)
}

pub fn set_routing_mode(mode: RoutingMode) -> Result<()> {
    IpcClient::new().set_routing_mode(mode)
}

pub fn refresh_subscription(sub_id: Option<String>) -> Result<()> {
    IpcClient::new().refresh_subscription(sub_id)
}

pub fn get_logs() -> Result<Vec<LogLine>> {
    IpcClient::new().get_logs()
}

pub fn get_processes() -> Result<Vec<RunningProcess>> {
    IpcClient::new().get_processes()
}

pub fn ping_server(server_id: &str) -> Result<Option<u32>> {
    IpcClient::new().ping_server(server_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_fallback_status() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("lightgui-settings-test-{ts}"));
        let client = IpcClient::with_pipe_and_dir(r"\\.\pipe\nonexistent_test_pipe", temp_dir.clone());

        // Status fallback should return disconnected state without panic
        let status = client.get_status().unwrap();
        assert_eq!(status.state.status, lightgui_core::models::ConnStatus::Disconnected);

        // Fallback switch_server updates settings
        client.switch_server("srv-test-1").unwrap();
        let settings = client.load_settings();
        assert_eq!(settings.selected_server_id.as_deref(), Some("srv-test-1"));

        // Fallback set_routing_mode updates settings
        client.set_routing_mode(RoutingMode::GlobalProxy).unwrap();
        let settings = client.load_settings();
        assert_eq!(settings.routing_mode, RoutingMode::GlobalProxy);

        // Fallback connect updates selected_server_id if Some
        client.connect(Some("srv-test-2".into())).unwrap();
        let settings = client.load_settings();
        assert_eq!(settings.selected_server_id.as_deref(), Some("srv-test-2"));

        // Fallback disconnect succeeds
        assert!(client.disconnect().is_ok());

        // Fallback get_logs returns empty vec
        let logs = client.get_logs().unwrap();
        assert!(logs.is_empty());

        // Fallback get_processes returns process list without panic
        let procs = client.get_processes().unwrap();
        assert!(!procs.is_empty());

        // Fallback ping_server for non-existent server returns None
        let ping = client.ping_server("srv-nonexistent").unwrap();
        assert_eq!(ping, None);

        // Fallback refresh_subscription succeeds
        assert!(client.refresh_subscription(None).is_ok());

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_client_settings_and_profiles_roundtrip() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("lightgui-settings-roundtrip-{ts}"));
        let client = IpcClient::with_pipe_and_dir(r"\\.\pipe\nonexistent_test_pipe", temp_dir.clone());

        let mut settings = client.load_settings();
        settings.mixed_port = 9090;
        settings.tun_mtu = 1500;
        client.save_settings(&settings).unwrap();

        let loaded = client.load_settings();
        assert_eq!(loaded.mixed_port, 9090);
        assert_eq!(loaded.tun_mtu, 1500);

        let mut profiles = client.load_profiles().unwrap();
        profiles.subscriptions.push(lightgui_core::models::Subscription {
            id: "sub-test".into(),
            name: "Test Subscription".into(),
            url: "https://example.com/sub".into(),
            updated_at: None,
            quota: None,
            auto_update_hours: 12,
            support_url: None,
            web_page_url: None,
            panel_title: None,
            servers: Vec::new(),
        });
        client.save_profiles(&profiles).unwrap();

        let loaded_profiles = client.load_profiles().unwrap();
        assert_eq!(loaded_profiles.subscriptions.len(), 1);
        assert_eq!(loaded_profiles.subscriptions[0].id, "sub-test");

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}


