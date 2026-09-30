use crate::models::{
    ConnectionState, LogLine, ProfileStore, RoutingMode, RunningProcess, Settings,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\umbralight_ipc";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    pub state: ConnectionState,
    pub active_server_name: Option<String>,
    pub uptime_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IpcRequest {
    GetStatus,
    Connect { server_id: Option<String> },
    Disconnect,
    SwitchServer { server_id: String },
    SetRoutingMode { mode: RoutingMode },
    SaveSettings { settings: Settings },
    SaveProfiles { profiles: ProfileStore },
    RefreshSubscription { sub_id: Option<String> },
    GetLogs,
    GetRunningProcesses,
    PingServer { server_id: String },
    PingAllServers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IpcResponse {
    Status(StatusInfo),
    Logs(Vec<LogLine>),
    Processes(Vec<RunningProcess>),
    PingResult { delay_ms: Option<u32> },
    PingAllResults { results: Vec<(String, Option<u32>)> },
    Success,
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CoreMode;

    #[test]
    fn test_ipc_request_roundtrip() {
        let req = IpcRequest::Connect {
            server_id: Some("srv-123".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("serverId"));
        let de: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, de);
        let switched: IpcRequest =
            serde_json::from_str(r#"{"type":"switchServer","payload":{"serverId":"srv-123"}}"#)
                .unwrap();
        assert_eq!(
            switched,
            IpcRequest::SwitchServer {
                server_id: "srv-123".into()
            }
        );
    }

    #[test]
    fn test_ipc_response_roundtrip() {
        let resp = IpcResponse::Status(StatusInfo {
            state: ConnectionState::disconnected(CoreMode::SystemProxy),
            active_server_name: None,
            uptime_secs: None,
        });
        let json = serde_json::to_string(&resp).unwrap();
        let de: IpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, de);
    }
}
