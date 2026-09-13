// autolined daemon — long-running background process serving suggestions over IPC

mod ipc;

use autoline_core::cascade::SuggestionCascade;
use clap::Parser;
use ipc::{run_ipc_server, ServerContext};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::info;
use tracing_subscriber::EnvFilter;

const MODEL_REBUILD_EVERY_N: usize = 50;

#[derive(Clone)]
pub struct DaemonState {
    pub cascade: Arc<Mutex<SuggestionCascade>>,
    pub model_rebuild_every_n: usize,
}

impl DaemonState {
    pub fn new(model_rebuild_every_n: usize) -> Self {
        let db_path = autoline_core::history::HistoryStore::default_path();
        let mut cascade = match autoline_core::history::HistoryStore::open(&db_path) {
            Ok(store) => {
                info!("Opened persistent history store at {:?}", db_path);
                SuggestionCascade::new().with_history(store)
            }
            Err(e) => {
                tracing::warn!("Failed to open persistent history store at {:?}: {}", db_path, e);
                SuggestionCascade::new()
            }
        };

        if let Ok(loaded) = cascade.load_from_history(5_000) {
            info!("Loaded {} history entries into suggestion engine", loaded);
        }

        Self {
            cascade: Arc::new(Mutex::new(cascade)),
            model_rebuild_every_n,
        }
    }
}

pub async fn run_daemon(model_rebuild_every_n: usize) -> anyhow::Result<()> {
    info!("Starting autolined daemon (core engine already running in autoline-core)");

    let state = DaemonState::new(model_rebuild_every_n);
    info!("Core suggestion engine active (trie + history + n-gram models)");

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let ctx = Arc::new(ServerContext {
        state,
        start_time: Instant::now(),
        shutdown_tx,
    });

    run_ipc_server(ctx, shutdown_rx).await?;
    info!("autolined shutdown complete");
    Ok(())
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
            let model_rebuild_every_n = cli.model_rebuild_every_n;
            run_daemon(model_rebuild_every_n).await?;
        }
        CliCommand::Stop => {
            info!("autolined stop requested (use autoline daemon stop)");
        }
        CliCommand::Status => {
            info!("autolined status requested (use autoline status)");
        }
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