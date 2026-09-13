# Autoline

> **Local-first, cross-shell inline autocomplete for terminal commands and AI prompts.**

Autoline learns from your command history and delivers sub-millisecond ghost-text completions directly inside your shell — no cloud required. It runs as a lightweight background daemon, persists history in a local SQLite database, and syncs across machines using CRDT-based merging over Git.

---

## Features

- ⚡ **Sub-millisecond suggestions** — Trie + N-gram cascade with fuzzy fallback
- 🧠 **Learns as you type** — Every command you run is recorded and ranked by frequency and recency
- 🔒 **Local-first & private** — All data stays on your machine in `~/.local/share/autoline/`
- 🔗 **IPC daemon architecture** — CLI talks to a background daemon over a Named Pipe (Windows) or Unix Domain Socket (Linux/macOS)
- 🌐 **Cross-machine sync** — CRDT history merging over Git, rsync, or HTTP backends
- 🐚 **Cross-shell support** — PowerShell, Bash, Zsh hooks included
- 📦 **Single install** — Two binaries: `autoline` (CLI) and `autolined` (daemon)

---

## Table of Contents

1. [Requirements](#requirements)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Shell Integration](#shell-integration)
5. [CLI Reference](#cli-reference)
6. [Architecture](#architecture)
7. [Cross-Machine Sync](#cross-machine-sync)
8. [Building from Source](#building-from-source)
9. [Troubleshooting](#troubleshooting)
10. [License](#license)

---

## Requirements

- **Rust** 1.70 or later (`rustup` recommended)
- **Windows** 10/11, **macOS** 12+, or **Linux** (kernel 5.x+)
- PowerShell 5.1+ (for Windows shell integration)

---

## Installation

### Install from source (recommended)

```bash
git clone https://github.com/user/autoline
cd autoline

# Install both binaries into ~/.cargo/bin (must be on your PATH)
cargo install --path crates/autoline-cli
cargo install --path crates/autolined
```

> **Important:** If a daemon is currently running, stop it before reinstalling:
> ```powershell
> autoline daemon stop
> cargo install --path crates/autolined
> ```

### Verify installation

```bash
autoline --version
autolined --version
```

---

## Quick Start

### 1. Initialize directories

```bash
autoline init
```

Creates `~/.local/share/autoline/` (or `%LOCALAPPDATA%\autoline\` on Windows) and `~/.config/autoline/`.

### 2. Start the daemon

Open a terminal and start the background daemon:

```bash
autolined start
```

> Only **one** daemon instance can run at a time. If you see `Access is denied (os error 5)`, check if a daemon is already running with `autoline status`.

### 3. Verify connection

```bash
autoline status
```

Expected output:
```
autoline status
  Daemon: connected (PID: 12345, uptime: 3s, version: 0.1.0)
  Models active: true
  History tracking: active (entries: 0)
```

### 4. Enable shell integration

See [Shell Integration](#shell-integration) below to enable live Tab completions.

---

## Shell Integration

The shell hook does two things automatically:
- **Tab** — queries the daemon for a ghost-text completion and inserts it
- **After each command** — silently records the executed command to history

### PowerShell (Windows)

Load the hook in your current session:

```powershell
autoline init --print-hook powershell | Out-String | Invoke-Expression
```

Make it **permanent** (runs on every new terminal):

```powershell
Add-Content $PROFILE "autolined start"
Add-Content $PROFILE "autoline init --print-hook powershell | Out-String | Invoke-Expression"
```

### Bash

Load the hook in your current session:

```bash
eval "$(autoline init --print-hook bash)"
```

Make it **permanent** (add to `~/.bashrc`):

```bash
echo 'eval "$(autoline init --print-hook bash)"' >> ~/.bashrc
```

### Zsh

Load the hook in your current session:

```zsh
eval "$(autoline init --print-hook zsh)"
```

Make it **permanent** (add to `~/.zshrc`):

```zsh
echo 'eval "$(autoline init --print-hook zsh)"' >> ~/.zshrc
```

### Using Tab completions

Once the hook is loaded:

1. Start typing a command, e.g. `cargo bu`
2. Press **Tab** — autoline inserts the predicted completion: `ild --release`
3. Press **Enter** to run — the command is automatically recorded

Suggestions improve over time as you build up command history. The ranking model boosts:
- Commands run **more frequently**
- Commands run **recently**
- Commands run in the **same project directory**

---

## CLI Reference

All commands assume `autoline` is on your `PATH`. If not, prefix with `.\target\release\autoline.exe` from the project root.

### `autoline status`
Show daemon connection status and history count.

```bash
autoline status
```

### `autoline suggest`
Query an inline suggestion for a given line buffer (used internally by shell hooks).

```bash
autoline suggest --line "cargo bu"
autoline suggest --line "git co" --cwd "/path/to/repo"
# Output: the completion suffix only, e.g. "ild --release"
```

### `autoline record`
Record a command into the history database.

```bash
autoline record --line "cargo build --release" --shell powershell
autoline record --line "git status" --shell bash --cwd "/path/to/project"
```

### `autoline init`
Initialize directories and print shell integration hooks.

```bash
autoline init                             # Create data/config directories
autoline init --print-hook powershell     # Print PowerShell hook script
autoline init --print-hook bash           # Print Bash hook script
autoline init --print-hook zsh            # Print Zsh hook script
```

### `autoline history`
Manage the local history database.

```bash
autoline history export    # Display all recorded commands in a table
autoline history clear     # Delete all history entries
```

> After `history clear`, restart the daemon to reload a clean model:
> ```bash
> autoline daemon stop
> autolined start
> ```

### `autoline sync`
Synchronize history across machines using CRDT merging.

```bash
autoline sync status   # Show backend info and local entry count
autoline sync push     # Push local history to sync repository
autoline sync pull     # Pull and merge remote history locally
```

### `autoline daemon`
Control the `autolined` background daemon.

```bash
autoline daemon status   # Check if daemon is running
autoline daemon stop     # Gracefully shut down the daemon
autoline daemon reload   # Reload model configuration
```

### `autoline project`
Manage per-project history contexts.

```bash
autoline project list          # List all autoline-tracked projects
autoline project prune         # Remove projects no longer on disk
autoline project rename <name> # Rename the current project
autoline project forget <id>   # Remove a specific project from tracking
```

---

## Architecture

Autoline is structured as a Cargo workspace with four crates:

```
autoline/
├── crates/
│   ├── autoline-core/     # Core library: Trie, N-gram, HistoryStore, cascade, protocol, sync
│   ├── autolined/         # Background daemon: IPC server, daemon state management
│   ├── autoline-cli/      # CLI binary: subcommands, shell hooks (PS1/bash/zsh)
│   └── autoline-cmd-wrap/ # Windows ConPTY wrapper for cmd.exe ghost-text support
```

### Suggestion Pipeline

When Tab is pressed, the request flows through a **cascade** of models, stopping at the first confident match:

```
User keypress (Tab)
    │
    ▼
Shell hook → autoline suggest --line "<buffer>"
    │
    ▼
IPC (Named Pipe / Unix Socket) → autolined daemon
    │
    ▼
SuggestionCascade:
  1. Trie prefix lookup        (exact prefix match, O(k))
  2. History prefix match      (SQLite, ranked by frequency × recency)
  3. N-gram prediction         (trigram → bigram → unigram fallback)
  4. Fuzzy match               (nucleo-powered approximate search)
    │
    ▼
Ghost text returned → inserted at cursor
```

### IPC Protocol

Messages are framed with a **4-byte big-endian length prefix** followed by **MessagePack**-encoded payloads. This keeps latency well under 1ms for local socket communication.

---

## Cross-Machine Sync

Autoline uses **CRDT (Conflict-free Replicated Data Type)** logic with **Lamport clocks** to merge command history across machines without conflicts.

### Setup with Git (default backend)

```bash
# Initialize a bare Git repository as your sync remote
git init --bare ~/.autoline_sync.git

# Push your history to it
autoline sync push

# On another machine, pull and merge
autoline sync pull
```

### How merging works

- Each history entry carries a Lamport timestamp
- On merge, entries with the same normalized command are compared by clock value — the higher clock wins
- Entries unique to either side are preserved
- The result is always deterministic and commutative regardless of merge order

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/user/autoline
cd autoline

# Build all crates in debug mode
cargo build --workspace

# Build optimized release binaries
cargo build --workspace --release

# Run all tests (90 unit + integration tests)
cargo test --workspace

# Run the daemon directly (development)
cargo run -p autolined --release -- start

# Run the CLI directly (development)
cargo run -p autoline-cli --release -- status
```

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info` | Log level for both `autoline` and `autolined` (e.g. `debug`, `warn`) |
| `AUTOLINE_DB` | `%LOCALAPPDATA%\autoline\history.db` | Override the SQLite database path |

---

## Troubleshooting

### `Access is denied (os error 5)` when starting daemon

A daemon instance is already running. Check and stop it first:

```bash
autoline status          # confirm it's connected
autoline daemon stop     # gracefully shut it down
autolined start          # start a fresh instance
```

### `autoline` not recognized after install

Ensure `~/.cargo/bin` is on your `PATH`:

```powershell
# PowerShell — add to $PROFILE permanently
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
```

```bash
# Bash/Zsh — add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"
```

### Tab always suggests the same word

The history database may contain a noisy entry (e.g. from loading the shell hook). Clear it and restart:

```bash
autoline history clear
autoline daemon stop
autolined start
autoline init --print-hook powershell | Out-String | Invoke-Expression
```

Then run a few real commands to seed useful history.

### Hook load error: `Unexpected token '?.Id'`

Your system runs **Windows PowerShell 5**, which doesn't support `?.` (null-conditional). Ensure you have the latest version of autoline installed:

```bash
cargo install --path crates/autoline-cli
```

### Suggestions not improving

Autoline learns from commands you execute through the hook. Make sure:
1. The daemon is running: `autoline status`
2. The hook is loaded: run `autoline init --print-hook powershell | Out-String | Invoke-Expression`
3. You've run enough commands for the model to learn from (10+ is a good start)

---

## License

MIT — see [LICENSE](LICENSE) for details.