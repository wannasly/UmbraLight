use serde::{Deserialize, Serialize};
use crate::models::{ConnectionState, LogLine, RoutingMode, RunningProcess};

pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\lightgui_ipc";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    pub state: ConnectionState,
    pub active_server_name: Option<String>,
    pub uptime_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum IpcRequest {
    GetStatus,
    Connect { server_id: Option<String> },
    Disconnect,
    SwitchServer { server_id: String },
    SetRoutingMode { mode: RoutingMode },
    RefreshSubscription { sub_id: Option<String> },
    GetLogs,
    GetRunningProcesses,
    PingServer { server_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum IpcResponse {
    Status(StatusInfo),
    Logs(Vec<LogLine>),
    Processes(Vec<RunningProcess>),
    PingResult { delay_ms: Option<u32> },
    Success,
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CoreMode;

    #[test]
    fn test_ipc_request_roundtrip() {
        let req = IpcRequest::Connect { server_id: Some("srv-123".into()) };
        let json = serde_json::to_string(&req).unwrap();
        let de: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, de);
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
