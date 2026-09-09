// Autoline IPC module — length-prefixed MessagePack framing over Unix socket (Linux/macOS)
// or Named Pipe (Windows).
//
// Transport:
//   - Linux/macOS: Unix domain socket at
//       $XDG_RUNTIME_DIR/autoline.sock (falls back to ~/.local/share/autoline/autoline.sock)
//   - Windows: Named pipe \\.\pipe\autoline
//
// Framing:
//   - Each message is prefixed with a 4-byte big-endian u32 length,
//     followed by the MessagePack-serialized payload.
//
// Message types (shared with autoline-core protocol.rs):
//   - SuggestRequest, SuggestResponse, RecordCommandPayload, Request, Response
//   - Implements Serde + MessagePack serialization/deserialization
//
// The daemon main.rs includes: run_daemon() which binds the socket
// and spawns one tokio task per connection.

use crate::cascade::SuggestionCascade;
use autoline_core::protocol::{self, IpcMessage, IpcResponseMsg, SuggestionSource};
use crate::DaemonState;
use anyhow::{Context, Result};
use fastnmp::Message;
use fastnmp::server::{Incoming, Server};
use serde::{Deserialize, Serialize};
use std::net::Shutdown;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

/// Unix domain socket path computation
pub fn socket_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join("autoline/autoline.sock"))
        .unwrap_or_else(|| std::path::PathBuf::from(".local/share/autoline/autoline.sock"))
}

/// A single connected client session
pub struct ClientSession {
    stream: std::net::UnixStream,
    session_id: String,
    cascade: Arc<RwLock<SuggestionCascade>>,
}

impl ClientSession {
    pub fn new(stream: std::net::UnixStream, cascade: Arc<RwLock<SuggestionCascade>>, session_id: String) -> anyhow::Result<Self> {
        Ok(Self {
            stream,
            session_id,
            cascade,
        })
    }

    /// Read one length-prefixed MessagePack message from the stream
    async fn read_message(&mut self) -> anyhow::Result<Message<protocol::Request>> {
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes).await
            .context("failed to read message length prefix")?;
        let message_len = u32::from_be_bytes(len_bytes) as usize;

        let mut msg_bytes = vec![0u8; message_len];
        self.stream.read_exact(&mut msg_bytes).await
            .context("failed to read message body")?;

        let msg = Message::try_from(msg_bytes.as_slice())
            .context("failed to deserialize MessagePack message")?;
        Ok(msg)
    }

    /// Send one length-prefixed MessagePack response to the stream
    async fn send_response(&mut self, response: &protocol::Response) -> anyhow::Result<()> {
        let bytes = match response {
            protocol::Response::Suggestion(resp) => {
                let msg = Message::try_from(resp).context("failed to serialize SuggestResponse")?;
                msg.to_vec()
            }
            protocol::Response::Ack => {
                let msg = Message::try_from(protocol::Response::Ack).context("failed to serialize Ack")?;
                msg.to_vec()
            }
            protocol::Response::Pong => {
                let msg = Message::try_from(protocol::Response::Pong).context("failed to serialize Pong")?;
                msg.to_vec()
            }
            protocol::Response::Error(msg) => {
                let msg = Message::try_from(protocol::Response::Error(msg.clone())).context("failed to serialize Error")?;
                msg.to_vec()
            }
        };

        let len = bytes.len();
        let len_bytes = (len as u32).to_be_bytes();
        self.stream.write_all(&len_bytes).await
            .context("failed to write length prefix")?;
        self.stream.write_all(&bytes).await
            .context("failed to write message body")?;
        Ok(())
    }

    /// Handle a single message from the client
    async fn handle_message(&mut self, msg: Message<protocol::Request>) -> anyhow::Result<()> {
        use protocol::Request::*;
        use protocol::Response::;

        let result = match msg.into_inner() {
            Suggest(req) => {
                let cascade = self.cascade.read().await;
                let resp = req.as_suggest(&*cascade);
                Suggestion(SuggestResponse {
                    suggestion: resp.suggestion,
                    source: resp.source,
                    confidence: resp.confidence,
                })
            }
            RecordCommand(payload) => {
                let mut cascade = self.cascade.write().await;
                cascade.train_ngram(req.kind, &req.line);
                Ack
            }
            Ping => Pong,
            ReloadModel => {
                Ack
            }
        };

        self.send_response(&result).await
    }

    /// Run the read loop for this client connection
    async fn run(mut self) -> anyhow::Result<()> {
        info!("Client connected: session={}", self.session_id);

        loop {
            let msg = match self.read_message().await {
                Ok(m) => m,
                Err(e) => {
                    warn!("Read error for session {}: {}", self.session_id, e);
                    break;
                }
            };

            if let Err(e) = self.handle_message(msg).await {
                error!("Handler error for session {}: {}", self.session_id, e);
                let _ = self.send_response(&Response::Error(e.to_string())).await;
            }
        }

        let _ = self.stream.shutdown(Shutdown::Both);
        info!("Client disconnected: session={}", self.session_id);
        Ok(())
    }
}

/// Start the Unix socket listener and accept connections
pub async fn start_listener(cascade: Arc<RwLock<SuggestionCascade>>) -> anyhow::Result<()> {
    let socket = socket_path();
    let _ = std::fs::remove_file(&socket);

    info!("Starting autolined Unix socket listener at {:?}", socket);

    let server = Server::new(socket.to_str().unwrap())
        .context("failed to create Unix socket server")?;

    info!("Unix socket listener active at {:?}", socket);

    loop {
        let incoming = server.incoming();
        let cascade = cascade.clone();

        tokio::spawn(async move {
            match incoming.await {
                Ok(stream) => {
                    let session_id = format!("session-{}-{}", std::time::Instant::now(), uuid::Uuid::v4());
                    let client = ClientSession::new(stream, cascade, session_id)
                        .unwrap_or_else(|e| {
                            warn!("Failed to create client session: {}", e);
                            return;
                        });

                    if let Err(e) = client.run().await {
                        warn!("Client session error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastnmp::Message;

    #[test]
    fn test_socket_path() {
        let path = socket_path();
        assert!(path.to_str().is_some());
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        use protocol::Request::Suggest;
        use protocol::{SuggestRequest, SuggestResponse, SuggestionSource};

        let req = SuggestRequest {
            session_id: "test-sess".to_string(),
            shell: protocol::protocol::ShellKind::Zsh,
            line: "git co".to_string(),
            cwd: "/home/user".to_string(),
        };

        let bytes = fastnmp::to_vec(&req).unwrap();
        let back: fastnmp::Message<protocol::Request> = fastnmp::from_bytes(&bytes).unwrap();

        match back.into_inner() {
            protocol::Request::Suggest(s) => {
                assert_eq!(s.session_id, "test-sess");
                assert_eq!(s.line, "git co");
            }
            _ => panic!("Expected Suggest variant"),
        }
    }
}