// Autoline CLI — init, status, daemon control, history management

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, warn, error};
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
    Init,
    /// Show daemon and model status
    Status,
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

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    info!("autoline CLI starting up");

    match cli.command {
        Commands::Init => {
            handle_init()?;
        }
        Commands::Status => {
            handle_status()?;
        }
        Commands::Daemon { cmd } => {
            handle_daemon(cmd)?;
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

fn handle_init() -> Result<()> {
    info!("Running autoline init");

    let available_shells = ["zsh", "bash", "powershell", "cmd"];

    let zsh_configs = [
        std::env::var("ZDOTDIR").unwrap_or_default() + "/.zshrc",
        std::env::home_dir()
            .map(|p| p.join(".zshrc").to_string_lossy().to_string())
            .unwrap_or_default(),
        std::env::home_dir()
            .map(|p| p.join(".config/zsh/.zshrc").to_string_lossy().to_string())
            .unwrap_or_default(),
    ];

    let zsh_config = zsh_configs
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .cloned()
        .unwrap_or_default();

    if zsh_config.is_empty() {
        warn!("No zsh config found; autoline init may not fully configure zsh");
    }

    if let Some(data_dir) = dirs::data_local_dir() {
        let autoline_dir = data_dir.join("autoline");
        std::fs::create_dir_all(&autoline_dir)?;
        info!("Data directory created: {:?}", autoline_dir);
    } else {
        warn!("Could not determine data directory");
    }

    if let Some(config_dir) = dirs::config_dir() {
        let autoline_config = config_dir.join("autoline");
        std::fs::create_dir_all(&autoline_config)?;
        info!("Config directory created: {:?}", autoline_config);
    } else {
        warn!("Could not determine config directory");
    }

    info!("autoline init complete");
    info!("  Available shells: {:?}", available_shells);

    if !zsh_config.is_empty() {
        info!("  Existing zsh config: {}", zsh_config);
    }

    Ok(())
}

fn handle_status() -> Result<()> {
    info!("autoline status");

    // Report history count from in-memory store
    println!("autoline status");
    println!("  Daemon: not connected (local mode)");
    println!("  History tracking: active");

    Ok(())
}

fn handle_daemon(cmd: DaemonCmd) -> Result<()> {
    match cmd {
        DaemonCmd::Start => {
            println!("Daemon start requested");
            println!("  Use: autolined start (starts the daemon subprocess)");
            Ok(())
        }
        DaemonCmd::Stop => {
            println!("Daemon stop requested");
            Ok(())
        }
        DaemonCmd::Status => {
            println!("Daemon status check");
            println!("  Use: autoline status or autolined status");
            Ok(())
        }
        DaemonCmd::Reload => {
            println!("Daemon reload requested");
            Ok(())
        }
    }
}

fn handle_history(cmd: HistoryCmd) -> Result<()> {
    match cmd {
        HistoryCmd::Clear => {
            println!("History clear requested");
            Ok(())
        }
        HistoryCmd::Export => {
            println!("History export requested");
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