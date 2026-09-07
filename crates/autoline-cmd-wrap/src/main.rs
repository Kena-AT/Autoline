use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "autoline-cmd-wrap", version, about = "Autoline cmd.exe ConPTY wrapper")]
struct Cli {
    /// Command to wrap (default: cmd.exe)
    #[arg(default_value = "cmd.exe")]
    command: String,
}

fn main() -> Result<()> {
    let _cli = Cli::parse();
    println!("autoline-cmd-wrap — skeleton only (ConPTY not yet implemented)");
    Ok(())
}
