use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use autoline_core::{cascade::SuggestionCascade, trie::Trie, ngram::PartitionedNgramModel, history::HistoryStore};
use autoline_core::protocol::ShellKind;
use std::collections::HashMap;

/// Benchmark trie lookup performance
fn trie_benchmark(c: &mut criterion::Criterion) {
    let mut trie = Trie::new();

    // Insert 10,000 common commands
    for i in 0..10000 {
        let cmd = format!("command_{}", i);
        trie.insert("command", &cmd, 1.0f32);
    }

    c.bench_function("trie_lookup_10k", |b| {
        b.iter(|| trie.lookup_prefix("command_5"))
    });

    c.bench_function("trie_insert_10k", |b| {
        b.iter(|| trie.insert("new_cmd", "new_cmd".to_string(), 0.5))
    });
}

/// Benchmark suggest latency with growing history
fn suggest_latency_benchmark(c: &mut criterion::Criterion) {
    let mut cascade = SuggestionCascade::new();
    let store = autoline_core::history::HistoryStore::in_memory().unwrap();
    cascade = cascade.with_history(store);

    // Train with increasing numbers of entries
    for size in [100, 1000, 10000] {
        let group_name = BenchmarkId::new("suggest_latency", size);

        for i in 0..size {
            let cmd = format!("git status_{}", i);
            let _ = cascade.history.as_mut().unwrap().insert(
                &cmd,
                autoline_core::history::HistoryKind::Command,
                None,
                None,
                None,
                None,
            );
        }

        c.bench_function(group_name, |b| {
            b.iter(|| cascade.suggest("git status", "/"));
        });
    }
}

/// Benchmark n-gram prediction
fn ngram_benchmark(c: &mut criterion::Criterion) {
    let mut model = autoline_core::ngram::PartitionedNgramModel::new();

    // Train with varied command sequences
    let trains = vec![
        "cargo build --release",
        "cargo test",
        "git commit -m",
        "npm install",
        "docker run",
    ];

    for train in &trains {
        let tokens: Vec<&str> = train.split_whitespace().collect();
        model.train_line(autoline_core::history::HistoryKind::Command, tokens.clone());
    }

    c.bench_function("ngram_predict", |b| {
        b.iter(|| model.predict_next(autoline_core::history::HistoryKind::Command, vec!["cargo"]))
    });
}

/// Benchmark project boost effect
fn project_boost_benchmark(c: &mut criterion::Criterion) {
    let mut cascade = SuggestionCascade::new();

    // Insert a trie entry
    cascade.insert_trie(
        autoline_core::history::HistoryKind::Command,
        "git checkout",
        "git checkout main".to_string(),
        0.9,
    );

    c.bench_function("project_boost_no_project", |b| {
        b.iter(|| black_box(cascade.suggest("git ch", "/home/user")))
    });

    // When in a project (simulated by using a path that detection would flag)
    // The boost is applied internally by cascade.suggest()
    c.bench_function("project_boost_with_project", |b| {
        b.iter(|| black_box(cascade.suggest("git ch", "/repo/src/project")))
    });
}

criterion_group!(
    name: all_benchmarks;
    trie_benchmark;
    suggest_latency_benchmark;
    ngram_benchmark;
    project_boost_benchmark
);

criterion_main!(all_benchmarks);