# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Project scaffolding: Cargo workspace with 4 crates (`autoline-core`, `autolined`, `autoline-cli`, `autoline-cmd-wrap`)
- `autoline-core`: Trie implementation with insert/lookup_prefix
- `autoline-core`: SQLite-backed history store with schema migrations
- `autoline-core`: N-gram model (bigram/trigram) with train/predict_next
- `autoline-core`: IPC protocol types with serde/MessagePack serialization
- `autoline-core`: Input classification (command vs. prompt) stub
- `autoline-core`: Suggestion cascade orchestrator stub
- Daemon and CLI binary skeletons with basic tokio async entrypoint
- MIT License, CHANGELOG, repository structure
