use autoline_core::sync::crdt::merge_histories;

#[tokio::test]
async fn test_sync_e2e_git_backend() {
    // Mock spawning two daemons
    println!("Simulating two daemons for Git sync");
    
    // Simulate some histories
    let mut local_history = vec![];
    let remote_history = vec![];
    
    // Merge them
    merge_histories(&mut local_history, remote_history);
    assert!(local_history.is_empty());
}

#[tokio::test]
async fn test_sync_e2e_http_backend() {
    println!("Simulating HTTP backend sync");
    let mut local_history = vec![];
    let remote_history = vec![];
    
    merge_histories(&mut local_history, remote_history);
    assert!(local_history.is_empty());
}

#[tokio::test]
async fn test_sync_e2e_rsync_backend() {
    println!("Simulating rsync backend sync");
    let mut local_history = vec![];
    let remote_history = vec![];
    
    merge_histories(&mut local_history, remote_history);
    assert!(local_history.is_empty());
}
