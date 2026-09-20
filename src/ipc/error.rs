use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IpcError {
    #[error("KiCad IPC socket was not found; open KiCad and enable the IPC API")]
    SocketNotFound,
    #[error("KiCad IPC transport error: {0}")]
    Transport(String),
    #[error("failed to encode or decode KiCad protobuf: {0}")]
    Codec(String),
    #[error("KiCad returned {status}: {message}")]
    Api { status: String, message: String },
    #[error("KiCad returned {actual}, expected {expected}")]
    UnexpectedResponse { expected: String, actual: String },
    #[error("KiCad response did not contain a message")]
    MissingResponse,
}

impl IpcError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Transport(message) => {
                let message = message.to_ascii_lowercase();
                message.contains("timed out")
                    || message.contains("timedout")
                    || message.contains("connection")
            }
            Self::Api { status, .. } => {
                matches!(status.as_str(), "AS_BUSY" | "AS_NOT_READY" | "AS_TIMEOUT")
            }
            _ => false,
        }
    }
}
