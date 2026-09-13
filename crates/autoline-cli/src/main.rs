// Autoline CLI — init, status, daemon control, history management

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "autoline", version, about = "Autoline — terminal autocomplete")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Detect shells and install shell integrations
    Init {
        /// Print shell hook script (powershell, bash, zsh)
        #[arg(long)]
        print_hook: Option<String>,
    },
    /// Show daemon and model status
    Status,
    /// Query inline suggestion from the running daemon
    Suggest {
        /// Current line buffer to suggest for
        #[arg(long)]
        line: String,
        /// Current working directory
        #[arg(long)]
        cwd: Option<String>,
    },
    /// Record an executed command into the daemon history
    Record {
        /// Executed command line
        #[arg(long)]
        line: String,
        /// Shell that executed the command (powershell, bash, zsh, cmd)
        #[arg(long)]
        shell: Option<String>,
        /// Current working directory
        #[arg(long)]
        cwd: Option<String>,
    },
    /// Synchronize history across machines (CRDT)
    Sync {
        #[command(subcommand)]
        cmd: SyncCmd,
    },
    /// Daemon control
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },
    /// History management
    History {
        #[command(subcommand)]
        cmd: HistoryCmd,
    },
    /// Project management
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
}

#[derive(Debug, Subcommand)]
enum SyncCmd {
    /// Pull remote changes and merge locally
    Pull,
    /// Push local changes to remote backend
    Push,
    /// Display sync status
    Status,
}

#[derive(Debug, Subcommand)]
enum DaemonCmd {
    Start,
    Stop,
    Status,
    Reload,
}

#[derive(Debug, Subcommand)]
enum HistoryCmd {
    Clear,
    Export,
}

#[derive(Debug, Subcommand)]
enum ProjectCmd {
    List,
    Rename {
        #[arg(value_name = "NEW_NAME")]
        new_name: String,
    },
    Forget {
        #[arg(value_name = "PROJECT_ID")]
        project_id: String,
    },
    Prune,
}

#[cfg(windows)]
pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\autoline";

#[cfg(unix)]
pub fn socket_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join("autoline/autoline.sock"))
        .unwrap_or_else(|| std::path::PathBuf::from(".local/share/autoline/autoline.sock"))
}

#[cfg(windows)]
async fn connect_to_daemon() -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let client = ClientOptions::new().open(WINDOWS_PIPE_NAME)?;
    Ok(client)
}

#[cfg(unix)]
async fn connect_to_daemon() -> Result<tokio::net::UnixStream> {
    let path = socket_path();
    let stream = tokio::net::UnixStream::connect(path).await?;
    Ok(stream)
}

async fn send_daemon_request(req: autoline_core::protocol::Request) -> Result<autoline_core::protocol::Response> {
    use autoline_core::protocol::{read_framed_msg, write_framed_msg};
    let mut stream = connect_to_daemon().await?;
    write_framed_msg(&mut stream, &req).await?;
    let resp = read_framed_msg(&mut stream).await?;
    Ok(resp)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .with_writer(std::io::stderr)  // logs → stderr so they don't pollute stdout pipes
        .init();

    let cli = Cli::parse();
    info!("autoline CLI starting up");

    match cli.command {
        Commands::Init { print_hook } => {
            handle_init(print_hook.as_deref())?;
        }
        Commands::Status => {
            handle_status().await?;
        }
        Commands::Suggest { line, cwd } => {
            handle_suggest(line, cwd).await?;
        }
        Commands::Record { line, shell, cwd } => {
            handle_record(line, shell.as_deref(), cwd).await?;
        }
        Commands::Sync { cmd } => {
            handle_sync(cmd)?;
        }
        Commands::Daemon { cmd } => {
            handle_daemon(cmd).await?;
        }
        Commands::History { cmd } => {
            handle_history(cmd)?;
        }
        Commands::Project { cmd } => {
            handle_project(cmd)?;
        }
    }

    Ok(())
}

fn handle_init(print_hook: Option<&str>) -> Result<()> {
    if let Some(shell) = print_hook {
        match shell.to_lowercase().as_str() {
            "powershell" | "pwsh" => {
                println!("{}", include_str!("hooks/powershell.ps1"));
                return Ok(());
            }
            "bash" => {
                println!("{}", include_str!("hooks/bash.sh"));
                return Ok(());
            }
            "zsh" => {
                println!("{}", include_str!("hooks/zsh.zsh"));
                return Ok(());
            }
            other => {
                eprintln!("Unsupported shell hook: {}", other);
                eprintln!("Supported shells: powershell, bash, zsh");
                std::process::exit(1);
            }
        }
    }

    info!("Running autoline init");

    let available_shells = ["zsh", "bash", "powershell", "cmd"];

    if let Some(data_dir) = dirs::data_local_dir() {
        let autoline_dir = data_dir.join("autoline");
        std::fs::create_dir_all(&autoline_dir)?;
        info!("Data directory ready: {:?}", autoline_dir);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let autoline_config = config_dir.join("autoline");
        std::fs::create_dir_all(&autoline_config)?;
        info!("Config directory ready: {:?}", autoline_config);
    }

    println!("autoline initialization complete.");
    println!("Supported shells: {}", available_shells.join(", "));
    println!();
    println!("To enable shell integration, add the corresponding hook to your profile:");
    println!("  PowerShell:  autoline init --print-hook powershell | Out-String | Invoke-Expression");
    println!("  Bash:        eval \"$(autoline init --print-hook bash)\"");
    println!("  Zsh:         eval \"$(autoline init --print-hook zsh)\"");

    Ok(())
}

async fn handle_suggest(line: String, cwd: Option<String>) -> Result<()> {
    let current_dir = cwd.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });

    let shell = if cfg!(windows) {
        autoline_core::history::ShellKind::PowerShell
    } else {
        autoline_core::history::ShellKind::Bash
    };

    let req = autoline_core::protocol::Request::Suggest(autoline_core::protocol::SuggestRequest {
        session_id: "cli".to_string(),
        shell,
        line: line.clone(),
        cwd: current_dir.clone(),
    });

    match send_daemon_request(req).await {
        Ok(autoline_core::protocol::Response::Suggestion(sug)) => {
            if let Some(ghost) = sug.suggestion {
                print!("{}", ghost);
            }
        }
        _ => {
            // Local fallback if daemon is unreachable
            let mut cascade = autoline_core::cascade::SuggestionCascade::new();
            let db_path = autoline_core::history::store::HistoryStore::default_path();
            if let Ok(store) = autoline_core::history::store::HistoryStore::open(db_path) {
                cascade = cascade.with_history(store);
                let _ = cascade.load_from_history(1000);
            }
            let resp = cascade.suggest(&line, &current_dir);
            if let Some(ghost) = resp.suggestion {
                print!("{}", ghost);
            }
        }
    }

    Ok(())
}

async fn handle_record(line: String, shell_str: Option<&str>, cwd: Option<String>) -> Result<()> {
    if line.trim().is_empty() {
        return Ok(());
    }

    let current_dir = cwd.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });

    let shell = shell_str
        .and_then(autoline_core::history::ShellKind::from_str)
        .unwrap_or(if cfg!(windows) {
            autoline_core::history::ShellKind::PowerShell
        } else {
            autoline_core::history::ShellKind::Bash
        });

    let req = autoline_core::protocol::Request::RecordCommand(autoline_core::protocol::RecordCommandPayload {
        line: line.clone(),
        kind: autoline_core::history::HistoryKind::Command,
        shell,
        cwd: current_dir.clone(),
        tool: None,
    });

    // Try sending to daemon first
    match send_daemon_request(req).await {
        Ok(autoline_core::protocol::Response::Ack) => Ok(()),
        _ => {
            // Fallback: write directly to local history SQLite
            let db_path = autoline_core::history::store::HistoryStore::default_path();
            if let Ok(mut store) = autoline_core::history::store::HistoryStore::open(db_path) {
                let _ = store.insert(&line, autoline_core::history::HistoryKind::Command, Some(shell), Some(&current_dir), None, None);
            }
            Ok(())
        }
    }
}

fn handle_sync(cmd: SyncCmd) -> Result<()> {
    let db_path = autoline_core::history::store::HistoryStore::default_path();
    let store = autoline_core::history::store::HistoryStore::open(db_path)?;

    // Use default git sync repo path ~/.autoline_sync.git
    let sync_repo = dirs::home_dir()
        .map(|h| h.join(".autoline_sync.git"))
        .unwrap_or_else(|| std::path::PathBuf::from(".autoline_sync.git"));

    let mut git_backend = autoline_core::sync::git::GitSyncBackend::new(sync_repo.clone());

    match cmd {
        SyncCmd::Status => {
            println!("autoline sync status");
            println!("  Backend: Git bare repository");
            println!("  Repository: {}", sync_repo.display());
            println!("  Enabled: {}", git_backend.is_enabled());
            let count = store.count().unwrap_or(0);
            println!("  Local history entries: {}", count);
        }
        SyncCmd::Push => {
            println!("Synchronizing history (push)...");
            let local_entries = store.get_synced_entries()?;
            println!("  Gathered {} local entries for sync.", local_entries.len());
            match git_backend.push() {
                Ok(autoline_core::sync::git::GitSyncResult::Success { entries_synced, new_lamport_clock }) => {
                    println!("  Sync push completed successfully (clock: {}, synced: {}).", new_lamport_clock, entries_synced);
                }
                Ok(autoline_core::sync::git::GitSyncResult::Failure { error }) => {
                    println!("  Sync push failed: {}", error);
                }
                Err(e) => {
                    println!("  Sync push error: {}", e);
                }
            }
        }
        SyncCmd::Pull => {
            println!("Synchronizing history (pull)...");
            match git_backend.pull() {
                Ok(autoline_core::sync::git::GitSyncResult::Success { entries_synced, new_lamport_clock }) => {
                    println!("  Sync pull completed successfully (clock: {}, synced: {}).", new_lamport_clock, entries_synced);
                }
                Ok(autoline_core::sync::git::GitSyncResult::Failure { error }) => {
                    println!("  Sync pull failed: {}", error);
                }
                Err(e) => {
                    println!("  Sync pull error: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn handle_status() -> Result<()> {
    info!("autoline status");

    println!("autoline status");
    match send_daemon_request(autoline_core::protocol::Request::Status).await {
        Ok(autoline_core::protocol::Response::Status(status)) => {
            println!("  Daemon: connected (PID: {}, uptime: {}s, version: {})", status.pid, status.uptime_secs, status.version);
            println!("  Models active: {}", status.models_active);
            println!("  History tracking: active (entries: {})", status.history_count);
        }
        Ok(autoline_core::protocol::Response::Pong) => {
            println!("  Daemon: connected (pong received)");
            println!("  History tracking: active");
        }
        _ => {
            println!("  Daemon: not connected (local mode)");
            println!("  History tracking: active");
        }
    }

    Ok(())
}

async fn handle_daemon(cmd: DaemonCmd) -> Result<()> {
    match cmd {
        DaemonCmd::Start => {
            println!("Daemon start requested");
            println!("  Use: autolined start (starts the daemon subprocess)");
            Ok(())
        }
        DaemonCmd::Stop => {
            println!("Daemon stop requested");
            match send_daemon_request(autoline_core::protocol::Request::Shutdown).await {
                Ok(_) => println!("  Daemon received shutdown command successfully."),
                Err(e) => println!("  Could not contact daemon to stop: {}", e),
            }
            Ok(())
        }
        DaemonCmd::Status => {
            println!("Daemon status check");
            match send_daemon_request(autoline_core::protocol::Request::Status).await {
                Ok(autoline_core::protocol::Response::Status(status)) => {
                    println!("  Daemon running (PID: {}, uptime: {}s, version: {})", status.pid, status.uptime_secs, status.version);
                }
                Ok(_) => {
                    println!("  Daemon responded with unexpected response.");
                }
                Err(e) => {
                    println!("  Daemon not reachable: {}", e);
                }
            }
            Ok(())
        }
        DaemonCmd::Reload => {
            println!("Daemon reload requested");
            match send_daemon_request(autoline_core::protocol::Request::ReloadModel).await {
                Ok(_) => println!("  Daemon models reload requested successfully."),
                Err(e) => println!("  Could not contact daemon to reload: {}", e),
            }
            Ok(())
        }
    }
}

fn handle_history(cmd: HistoryCmd) -> Result<()> {
    let db_path = autoline_core::history::store::HistoryStore::default_path();
    match cmd {
        HistoryCmd::Clear => {
            let mut store = autoline_core::history::store::HistoryStore::open(db_path)?;
            let deleted = store.clear()?;
            println!("History cleared: {} entries removed.", deleted);
            println!("Tip: restart the daemon so it reloads a clean model:");
            println!("  autoline daemon stop && autolined start");
            Ok(())
        }
        HistoryCmd::Export => {
            let store = autoline_core::history::store::HistoryStore::open(db_path)?;
            let rows = store.all(10_000)?;
            if rows.is_empty() {
                println!("No history entries found.");
            } else {
                println!("{:<5}  {:<12}  {:<10}  {}", "Count", "Shell", "Kind", "Command");
                println!("{}", "-".repeat(72));
                for row in &rows {
                    println!(
                        "{:<5}  {:<12}  {:<10}  {}",
                        row.used_count,
                        row.shell.map(|s| s.as_str()).unwrap_or("unknown"),
                        row.kind.as_str(),
                        row.line
                    );
                }
                println!("\nTotal: {} entries", rows.len());
            }
            Ok(())
        }
    }
}

fn handle_project(cmd: ProjectCmd) -> Result<()> {
    match cmd {
        ProjectCmd::List => {
            if let Some(data_dir) = dirs::data_local_dir() {
                let autoline_dir = data_dir.join("autoline");
                if autoline_dir.exists() {
                    println!("Project directories under: {:?}", autoline_dir);
                    // List subdirectories that look like projects
                    if let Ok(entries) = std::fs::read_dir(&autoline_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir() && path.join(".autoline.project").exists() {
                                println!("  - {} (autoline project)", path.display());
                            }
                        }
                    }
                } else {
                    warn!("Data directory not found");
                }
            } else {
                warn!("Could not determine data directory");
            }
            Ok(())
        }
        ProjectCmd::Rename { new_name } => {
            if let Some(data_dir) = dirs::data_local_dir() {
                let autoline_dir = data_dir.join("autoline");
                if autoline_dir.exists() {
                    println!("Rename project to: {}", new_name);
                    // TODO: Implement actual rename logic
                    println!("  (rename not yet implemented)");
                } else {
                    warn!("Data directory not found");
                }
            } else {
                warn!("Could not determine data directory");
            }
            Ok(())
        }
        ProjectCmd::Forget { project_id } => {
            if let Some(data_dir) = dirs::data_local_dir() {
                let autoline_dir = data_dir.join("autoline");
                if autoline_dir.exists() {
                    println!("Forget project: {}", project_id);
                    // TODO: Implement actual forget logic
                    println!("  (forget not yet implemented)");
                } else {
                    warn!("Data directory not found");
                }
            } else {
                warn!("Could not determine data directory");
            }
            Ok(())
        }
        ProjectCmd::Prune => {
            if let Some(data_dir) = dirs::data_local_dir() {
                let autoline_dir = data_dir.join("autoline");
                if autoline_dir.exists() {
                    println!("Prune removed projects");
                    // TODO: Implement prune logic
                    println!("  (prune not yet implemented)");
                } else {
                    warn!("Data directory not found");
                }
            } else {
                warn!("Could not determine data directory");
            }
            Ok(())
        }
    }
}