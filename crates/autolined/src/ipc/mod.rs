// Autoline IPC module — length-prefixed MessagePack framing over Unix socket (Linux/macOS)
// or Named Pipe (Windows).

use autoline_core::protocol::{
    read_framed_msg, write_framed_msg, DaemonStatusResponse, Request, Response,
};
use crate::DaemonState;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

#[cfg(windows)]
pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\autoline";

/// Socket path computation for Unix systems
#[cfg(unix)]
pub fn socket_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join("autoline/autoline.sock"))
        .unwrap_or_else(|| std::path::PathBuf::from(".local/share/autoline/autoline.sock"))
}

pub struct ServerContext {
    pub state: DaemonState,
    pub start_time: Instant,
    pub shutdown_tx: broadcast::Sender<()>,
}

pub async fn handle_client_stream<S>(
    mut stream: S,
    ctx: Arc<ServerContext>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let req: Request = match read_framed_msg(&mut stream).await {
            Ok(r) => r,
            Err(_) => {
                // Client disconnected or EOF
                break;
            }
        };

        let resp = match req {
            Request::Ping => Response::Pong,
            Request::Status => {
                let history_count = {
                    let cascade = ctx.state.cascade.lock().unwrap();
                    cascade.history().and_then(|h| h.count().ok()).unwrap_or(0)
                };
                Response::Status(DaemonStatusResponse {
                    pid: std::process::id(),
                    uptime_secs: ctx.start_time.elapsed().as_secs(),
                    history_count,
                    models_active: true,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                })
            }
            Request::Suggest(suggest_req) => {
                let sug = {
                    let cascade = ctx.state.cascade.lock().unwrap();
                    cascade.suggest_from_request(&suggest_req)
                };
                Response::Suggestion(sug)
            }
            Request::RecordCommand(payload) => {
                {
                    let mut cascade = ctx.state.cascade.lock().unwrap();
                    let _ = cascade.record(
                        &payload.line,
                        payload.kind,
                        Some(payload.shell),
                        Some(&payload.cwd),
                        payload.tool.as_deref(),
                        None,
                    );
                }
                Response::Ack
            }
            Request::ReloadModel => {
                Response::Ack
            }
            Request::Shutdown => {
                let _ = write_framed_msg(&mut stream, &Response::Ack).await;
                let _ = ctx.shutdown_tx.send(());
                break;
            }
        };

        if let Err(e) = write_framed_msg(&mut stream, &resp).await {
            warn!("Failed to send response to client: {}", e);
            break;
        }
    }
    Ok(())
}

#[cfg(windows)]
pub async fn run_ipc_server(
    ctx: Arc<ServerContext>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    info!("Starting autolined Windows Named Pipe server at {}", WINDOWS_PIPE_NAME);

    // Initial server instance
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(WINDOWS_PIPE_NAME)
        .context("Failed to create initial named pipe instance")?;

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("IPC server received shutdown signal");
                break;
            }
            res = server.connect() => {
                match res {
                    Ok(()) => {
                        let connected_client = server;
                        match ServerOptions::new().create(WINDOWS_PIPE_NAME) {
                            Ok(next_server) => {
                                server = next_server;
                            }
                            Err(e) => {
                                error!("Failed to create next named pipe instance: {}", e);
                                break;
                            }
                        }

                        let client_ctx = ctx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client_stream(connected_client, client_ctx).await {
                                warn!("Error in client handler: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Error connecting to named pipe client: {}", e);
                        if let Ok(next_server) = ServerOptions::new().create(WINDOWS_PIPE_NAME) {
                            server = next_server;
                        }
                    }
                }
            }
        }
    }

    info!("IPC server stopped");
    Ok(())
}

#[cfg(unix)]
pub async fn run_ipc_server(
    ctx: Arc<ServerContext>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    use tokio::net::UnixListener;

    let path = socket_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&path);

    info!("Starting autolined Unix socket listener at {:?}", path);
    let listener = UnixListener::bind(&path).context("Failed to bind Unix socket")?;

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("IPC server received shutdown signal");
                break;
            }
            res = listener.accept() => {
                match res {
                    Ok((stream, _addr)) => {
                        let client_ctx = ctx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client_stream(stream, client_ctx).await {
                                warn!("Error in client handler: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Error accepting Unix connection: {}", e);
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(&path);
    info!("IPC server stopped");
    Ok(())
}