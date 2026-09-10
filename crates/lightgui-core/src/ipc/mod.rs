pub mod pipe;
pub mod protocol;

pub use pipe::{read_frame, write_frame};
pub use protocol::{IpcRequest, IpcResponse, StatusInfo, DEFAULT_PIPE_NAME};
