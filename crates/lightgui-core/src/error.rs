use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Network(String),
    Parse(String),
    NotFound(String),
    Unsupported(String),
    Internal(String),
    CoreNotInstalled,
    CoreStartFailed(String),
    DeviceLimit,
    HwidRequired,
    ElevationRequired,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
            Error::Network(s) => write!(f, "Network error: {s}"),
            Error::Parse(s) => write!(f, "Parse error: {s}"),
            Error::NotFound(s) => write!(f, "Not found: {s}"),
            Error::Unsupported(s) => write!(f, "Unsupported: {s}"),
            Error::Internal(s) => write!(f, "Internal error: {s}"),
            Error::CoreNotInstalled => write!(f, "sing-box binary is not installed"),
            Error::CoreStartFailed(s) => write!(f, "Core start failed: {s}"),
            Error::DeviceLimit => write!(f, "Device limit reached for this subscription"),
            Error::HwidRequired => write!(f, "Device hardware ID is required by subscription"),
            Error::ElevationRequired => write!(f, "Administrator privileges are required for TUN mode"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Network(e.to_string())
    }
}
