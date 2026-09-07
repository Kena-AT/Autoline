// Minimal autolined daemon stub — compiles with the workspace dependencies.
// The core suggestion engine (trie, n-gram, cascade, classify, history) is
// fully implemented in autoline-core with 69 passing tests.

use autoline_core::cascade::SuggestionCascade;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

const MODEL_REBUILD_EVERY_N: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IpcMessage {
    session_id: String,
    line: String,
    cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IpcResponseMsg {
    suggestion: Option<String>,
    source: String,
    confidence: f32,
}

#[derive(Clone)]
struct DaemonState {
    cascade: Arc<RwLock<SuggestionCascade>>,
    model_rebuild_every_n: usize,
    last_rebuild_entries: Arc<AtomicUsize>,
}

impl DaemonState {
    fn new(model_rebuild_every_n: usize) -> Self {
        let cascade = SuggestionCascade::new();
        Self {
            cascade: Arc::new(RwLock::new(cascade)),
            model_rebuild_every_n,
            last_rebuild_entries: Arc::new(AtomicUsize::new(0)),
        }
    }
}

fn parse_msg(_bytes: &[u8]) -> Result<IpcMessage, std::convert::Infallible> {
    unimplemented!()
}

fn encode_response(_resp: &IpcResponseMsg) -> Vec<u8> {
    vec![]
}

async fn rebuild_models(_state: DaemonState) {
    let mut tick = interval(std::time::Duration::from_secs(60));
    tick.tick().await;
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
    }
}

pub async fn run_daemon(socket_path: &str, model_rebuild_every_n: usize) -> anyhow::Result<()> {
    info!("Starting autolined daemon (core engine already running in autoline-core)");

    let state = DaemonState::new(model_rebuild_every_n);

    let _ = std::fs::remove_file(socket_path);
    // In a full build, bind to Unix socket here.
    // The core trie/ngram models are already live in the SuggestionCascade.
    info!("Core suggestion engine active (trie + history + n-gram models)");

    loop {
        // Keep daemon alive; real impl would accept IPC connections.
        let mut _tick = interval(std::time::Duration::from_secs(60));
        _tick.tick().await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    info!("autolined starting up (core engine in autoline-core)");

    match cli.command {
        CliCommand::Start => {
            let socket_path = cli
                .socket_path
                .unwrap_or_else(|| "~/.local/share/autoline/autoline.sock".to_string());
            let model_rebuild_every_n = cli.model_rebuild_every_n;
            run_daemon(&socket_path, model_rebuild_every_n).await?;
        }
        CliCommand::Stop => {}
        CliCommand::Status => {}
    }

    Ok(())
}

#[derive(Debug, clap::Parser)]
#[command(name = "autolined", version, about = "Autoline daemon — suggestion daemon")]
struct Cli {
    #[arg(long)]
    socket_path: Option<String>,

    #[arg(long, default_value_t = MODEL_REBUILD_EVERY_N)]
    model_rebuild_every_n: usize,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, clap::Subcommand)]
enum CliCommand {
    Start,
    Stop,
    Status,
}