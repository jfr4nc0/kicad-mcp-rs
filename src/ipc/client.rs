use crate::{
    discovery,
    ipc::error::IpcError,
    proto::v10::kiapi::common::{ApiRequest, ApiRequestHeader, ApiResponse, ApiStatusCode},
};
use nng::{
    Protocol, Socket,
    options::{Options, RecvMaxSize, RecvTimeout, SendTimeout},
};
use prost::Message;
use prost_types::Any;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use uuid::Uuid;

const TYPE_PREFIX: &str = "type.googleapis.com/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallPolicy {
    ReadOnly,
    Mutation,
}

#[derive(Debug, Clone)]
pub struct IpcConfig {
    pub socket_path: Option<PathBuf>,
    pub timeout: Duration,
    pub max_response_bytes: usize,
    pub read_retries: usize,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            socket_path: discovery::discover_socket().0,
            timeout: Duration::from_secs(5),
            max_response_bytes: 8 * 1024 * 1024,
            read_retries: 3,
        }
    }
}

#[derive(Clone)]
pub struct IpcClient {
    inner: Arc<Mutex<ClientState>>,
    config: IpcConfig,
    client_name: String,
}

struct ClientState {
    socket: Option<Socket>,
    token: String,
    socket_path: Option<PathBuf>,
}

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IpcClient")
            .field("config", &self.config)
            .field("client_name", &self.client_name)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl IpcClient {
    pub fn new(config: IpcConfig) -> Self {
        let socket_path = config.socket_path.clone();
        Self {
            inner: Arc::new(Mutex::new(ClientState {
                socket: None,
                token: std::env::var("KICAD_API_TOKEN").unwrap_or_default(),
                socket_path,
            })),
            config,
            client_name: format!("com.github.jfr4nc0.kicad-mcp-{}", Uuid::new_v4()),
        }
    }

    pub fn configured_socket(&self) -> Option<PathBuf> {
        self.inner
            .lock()
            .expect("IPC client mutex poisoned")
            .socket_path
            .clone()
    }

    pub fn token_present(&self) -> bool {
        !self
            .inner
            .lock()
            .expect("IPC client mutex poisoned")
            .token
            .is_empty()
    }

    pub fn disconnect(&self) {
        let mut state = self.inner.lock().expect("IPC client mutex poisoned");
        state.socket = None;
        let current_missing = state.socket_path.as_ref().is_none_or(|path| !path.exists());
        if current_missing {
            state.socket_path = discovery::discover_socket()
                .0
                .or_else(|| self.config.socket_path.clone());
        }
    }

    pub fn call<Q, R>(
        &self,
        request: &Q,
        request_type: &str,
        response_type: &str,
        policy: CallPolicy,
    ) -> Result<R, IpcError>
    where
        Q: Message,
        R: Message + Default,
    {
        let attempts = match policy {
            CallPolicy::ReadOnly => self.config.read_retries.max(1),
            CallPolicy::Mutation => 1,
        };

        let mut last_error = None;
        for attempt in 0..attempts {
            match self.call_once(request, request_type, response_type) {
                Ok(response) => return Ok(response),
                Err(error) if policy == CallPolicy::ReadOnly && error.is_transient() => {
                    last_error = Some(error);
                    self.disconnect();
                    if attempt + 1 < attempts {
                        thread::sleep(Duration::from_millis(100 * (1_u64 << attempt.min(4))));
                    }
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or(IpcError::MissingResponse))
    }

    fn call_once<Q, R>(
        &self,
        request: &Q,
        request_type: &str,
        response_type: &str,
    ) -> Result<R, IpcError>
    where
        Q: Message,
        R: Message + Default,
    {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| IpcError::Transport("IPC client mutex poisoned".to_string()))?;

        if state.socket_path.is_none() {
            state.socket_path = discovery::discover_socket().0;
        }
        let socket_path = state.socket_path.clone().ok_or(IpcError::SocketNotFound)?;

        if state.socket.is_none() {
            state.socket = Some(self.connect(&socket_path)?);
        }

        let request_token = state.token.clone();
        let envelope = ApiRequest {
            header: Some(ApiRequestHeader {
                kicad_token: state.token.clone(),
                client_name: self.client_name.clone(),
            }),
            message: Some(Any {
                type_url: type_url(request_type),
                value: request.encode_to_vec(),
            }),
        };

        let socket = state.socket.as_ref().ok_or(IpcError::SocketNotFound)?;
        socket
            .send(envelope.encode_to_vec().as_slice())
            .map_err(|(_, error)| IpcError::Transport(error.to_string()))?;
        let message = socket
            .recv()
            .map_err(|error| IpcError::Transport(error.to_string()))?;
        let response = ApiResponse::decode(&message[..])
            .map_err(|error| IpcError::Codec(error.to_string()))?;

        let response_header = response.header.ok_or(IpcError::MissingResponse)?;
        let response_token = response_header.kicad_token;
        if !response_token.is_empty() {
            state.token = response_token;
        }

        let status = response.status.ok_or(IpcError::MissingResponse)?;
        let status_code =
            ApiStatusCode::try_from(status.status).unwrap_or(ApiStatusCode::AsUnknown);
        if status_code != ApiStatusCode::AsOk {
            return Err(IpcError::Api {
                status: status_code.as_str_name().to_string(),
                message: redact_pair(&status.error_message, &request_token, &state.token),
            });
        }

        let inner = response.message.ok_or(IpcError::MissingResponse)?;
        let expected = type_url(response_type);
        if inner.type_url != expected {
            return Err(IpcError::UnexpectedResponse {
                expected,
                actual: inner.type_url,
            });
        }

        R::decode(inner.value.as_slice()).map_err(|error| IpcError::Codec(error.to_string()))
    }

    fn connect(&self, path: &Path) -> Result<Socket, IpcError> {
        let socket =
            Socket::new(Protocol::Req0).map_err(|error| IpcError::Transport(error.to_string()))?;
        socket
            .set_opt::<RecvTimeout>(Some(self.config.timeout))
            .map_err(|error| IpcError::Transport(error.to_string()))?;
        socket
            .set_opt::<SendTimeout>(Some(self.config.timeout))
            .map_err(|error| IpcError::Transport(error.to_string()))?;
        socket
            .set_opt::<RecvMaxSize>(self.config.max_response_bytes)
            .map_err(|error| IpcError::Transport(error.to_string()))?;
        socket
            .dial(&socket_url(path))
            .map_err(|error| IpcError::Transport(error.to_string()))?;
        Ok(socket)
    }
}

pub fn type_url(full_name: &str) -> String {
    format!("{TYPE_PREFIX}{full_name}")
}

pub fn socket_url(path: &Path) -> String {
    let value = path.to_string_lossy();
    if value.starts_with("ipc://") {
        value.into_owned()
    } else {
        format!("ipc://{value}")
    }
}

fn redact(message: &str, token: &str) -> String {
    if token.is_empty() {
        message.to_string()
    } else {
        message.replace(token, "[REDACTED]")
    }
}

fn redact_pair(message: &str, request_token: &str, response_token: &str) -> String {
    redact(&redact(message, request_token), response_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_type_urls() {
        assert_eq!(
            type_url("kiapi.common.commands.GetVersion"),
            "type.googleapis.com/kiapi.common.commands.GetVersion"
        );
    }

    #[test]
    fn normalizes_socket_paths() {
        assert_eq!(
            socket_url(Path::new("/tmp/kicad/api.sock")),
            "ipc:///tmp/kicad/api.sock"
        );
        assert_eq!(
            socket_url(Path::new("ipc:///tmp/kicad/api.sock")),
            "ipc:///tmp/kicad/api.sock"
        );
    }

    #[test]
    fn mutation_errors_are_not_marked_for_retry() {
        let error = IpcError::Api {
            status: "AS_BAD_REQUEST".to_string(),
            message: "invalid".to_string(),
        };
        assert!(!error.is_transient());
        let mismatch = IpcError::Api {
            status: "AS_TOKEN_MISMATCH".to_string(),
            message: "rotated".to_string(),
        };
        assert!(mismatch.is_transient());
    }

    #[test]
    fn exchanges_protobuf_over_nng_req_rep() {
        use crate::proto::v10::kiapi::{
            common::commands::{GetVersion, GetVersionResponse},
            common::types::KiCadVersion,
            common::{ApiResponseHeader, ApiResponseStatus},
        };

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("api.sock");
        let listener = Socket::new(Protocol::Rep0).unwrap();
        listener.listen(&socket_url(&path)).unwrap();

        let server = std::thread::spawn(move || {
            let request = listener.recv().unwrap();
            let envelope = ApiRequest::decode(&request[..]).unwrap();
            assert_eq!(
                envelope.message.unwrap().type_url,
                type_url("kiapi.common.commands.GetVersion")
            );
            let version = GetVersionResponse {
                version: Some(KiCadVersion {
                    major: 10,
                    minor: 0,
                    patch: 6,
                    full_version: "10.0.6".to_string(),
                }),
            };
            let response = ApiResponse {
                header: Some(ApiResponseHeader {
                    kicad_token: "test-secret".to_string(),
                }),
                status: Some(ApiResponseStatus {
                    status: ApiStatusCode::AsOk as i32,
                    error_message: String::new(),
                }),
                message: Some(Any {
                    type_url: type_url("kiapi.common.commands.GetVersionResponse"),
                    value: version.encode_to_vec(),
                }),
            };
            listener.send(response.encode_to_vec().as_slice()).unwrap();
        });

        let client = IpcClient::new(IpcConfig {
            socket_path: Some(path),
            timeout: Duration::from_secs(2),
            max_response_bytes: 1024 * 1024,
            read_retries: 1,
        });
        let response: GetVersionResponse = client
            .call(
                &GetVersion {},
                "kiapi.common.commands.GetVersion",
                "kiapi.common.commands.GetVersionResponse",
                CallPolicy::ReadOnly,
            )
            .unwrap();
        assert_eq!(response.version.unwrap().full_version, "10.0.6");
        assert!(client.token_present());
        server.join().unwrap();
    }

    #[test]
    fn retries_transient_read_status_once() {
        use crate::proto::v10::kiapi::{
            common::commands::{GetVersion, GetVersionResponse},
            common::types::KiCadVersion,
            common::{ApiResponseHeader, ApiResponseStatus},
        };

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("api.sock");
        let listener = Socket::new(Protocol::Rep0).unwrap();
        listener.listen(&socket_url(&path)).unwrap();
        let server = std::thread::spawn(move || {
            let _ = listener.recv().unwrap();
            listener
                .send(
                    ApiResponse {
                        header: Some(ApiResponseHeader {
                            kicad_token: "rotated-secret".to_string(),
                        }),
                        status: Some(ApiResponseStatus {
                            status: ApiStatusCode::AsBusy as i32,
                            error_message: "busy".to_string(),
                        }),
                        message: None,
                    }
                    .encode_to_vec()
                    .as_slice(),
                )
                .unwrap();
            let retry = listener.recv().unwrap();
            let retry = ApiRequest::decode(&retry[..]).unwrap();
            assert_eq!(retry.header.unwrap().kicad_token, "rotated-secret");
            let version = GetVersionResponse {
                version: Some(KiCadVersion {
                    major: 10,
                    minor: 0,
                    patch: 6,
                    full_version: "10.0.6".to_string(),
                }),
            };
            listener
                .send(
                    ApiResponse {
                        header: Some(ApiResponseHeader {
                            kicad_token: "rotated-secret".to_string(),
                        }),
                        status: Some(ApiResponseStatus {
                            status: ApiStatusCode::AsOk as i32,
                            error_message: String::new(),
                        }),
                        message: Some(Any {
                            type_url: type_url("kiapi.common.commands.GetVersionResponse"),
                            value: version.encode_to_vec(),
                        }),
                    }
                    .encode_to_vec()
                    .as_slice(),
                )
                .unwrap();
        });
        let client = IpcClient::new(IpcConfig {
            socket_path: Some(path),
            timeout: Duration::from_secs(1),
            max_response_bytes: 1024,
            read_retries: 2,
        });
        let response: GetVersionResponse = client
            .call(
                &GetVersion {},
                "kiapi.common.commands.GetVersion",
                "kiapi.common.commands.GetVersionResponse",
                CallPolicy::ReadOnly,
            )
            .unwrap();
        assert_eq!(response.version.unwrap().full_version, "10.0.6");
        server.join().unwrap();
    }

    #[test]
    fn malformed_response_is_rejected() {
        use crate::proto::v10::kiapi::common::commands::{GetVersion, GetVersionResponse};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("api.sock");
        let listener = Socket::new(Protocol::Rep0).unwrap();
        listener.listen(&socket_url(&path)).unwrap();
        let server = std::thread::spawn(move || {
            let _ = listener.recv().unwrap();
            listener.send(&[0xff, 0xff][..]).unwrap();
        });
        let client = IpcClient::new(IpcConfig {
            socket_path: Some(path),
            timeout: Duration::from_secs(1),
            max_response_bytes: 1024,
            read_retries: 1,
        });
        let error = client
            .call::<_, GetVersionResponse>(
                &GetVersion {},
                "kiapi.common.commands.GetVersion",
                "kiapi.common.commands.GetVersionResponse",
                CallPolicy::ReadOnly,
            )
            .unwrap_err();
        assert!(matches!(error, IpcError::Codec(_)));
        server.join().unwrap();
    }

    #[test]
    fn timeout_is_transient_for_reads() {
        use crate::proto::v10::kiapi::common::commands::{GetVersion, GetVersionResponse};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("api.sock");
        let listener = Socket::new(Protocol::Rep0).unwrap();
        listener.listen(&socket_url(&path)).unwrap();
        let server = std::thread::spawn(move || {
            let _ = listener.recv().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        });
        let client = IpcClient::new(IpcConfig {
            socket_path: Some(path),
            timeout: Duration::from_millis(20),
            max_response_bytes: 1024,
            read_retries: 1,
        });
        let error = client
            .call::<_, GetVersionResponse>(
                &GetVersion {},
                "kiapi.common.commands.GetVersion",
                "kiapi.common.commands.GetVersionResponse",
                CallPolicy::ReadOnly,
            )
            .unwrap_err();
        assert!(error.is_transient(), "unexpected timeout error: {error}");
        server.join().unwrap();
    }

    #[test]
    fn token_is_redacted() {
        assert_eq!(redact("bad token secret", "secret"), "bad token [REDACTED]");
        assert_eq!(
            redact_pair("old-secret and new-secret", "old-secret", "new-secret"),
            "[REDACTED] and [REDACTED]"
        );
    }
}
