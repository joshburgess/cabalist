use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use cabalist_lsp::state::DocumentState;

type DocumentMap = Arc<RwLock<HashMap<String, DocumentState>>>;

fn sample_cabal(name: &str) -> String {
    format!(
        "cabal-version: 3.0\nname: {name}\nversion: 0.1.0.0\nlicense: MIT\n"
    )
}

fn make_docs() -> DocumentMap {
    Arc::new(RwLock::new(HashMap::new()))
}

#[tokio::test]
async fn concurrent_opens_on_different_documents() {
    let docs = make_docs();

    let mut handles = Vec::new();
    for i in 0..50 {
        let docs = docs.clone();
        handles.push(tokio::spawn(async move {
            let uri = format!("file:///project/pkg{i}.cabal");
            let source = sample_cabal(&format!("pkg{i}"));
            let mut map = docs.write().await;
            map.insert(uri, DocumentState::new(source, 1));
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let map = docs.read().await;
    assert_eq!(map.len(), 50);
}

#[tokio::test]
async fn concurrent_reads_during_writes() {
    let docs = make_docs();

    // Pre-populate 10 documents.
    {
        let mut map = docs.write().await;
        for i in 0..10 {
            let uri = format!("file:///project/pkg{i}.cabal");
            map.insert(uri, DocumentState::new(sample_cabal(&format!("pkg{i}")), 1));
        }
    }

    let mut handles = Vec::new();

    // Spawn writers that update documents.
    for version in 2..=20 {
        let docs = docs.clone();
        handles.push(tokio::spawn(async move {
            let mut map = docs.write().await;
            for i in 0..10 {
                let uri = format!("file:///project/pkg{i}.cabal");
                if let Some(doc) = map.get_mut(&uri) {
                    let new_source = format!(
                        "cabal-version: 3.0\nname: pkg{i}\nversion: 0.{version}.0.0\nlicense: MIT\n"
                    );
                    doc.update(new_source, version);
                }
            }
        }));
    }

    // Spawn readers that check documents concurrently.
    for _ in 0..50 {
        let docs = docs.clone();
        handles.push(tokio::spawn(async move {
            let map = docs.read().await;
            for i in 0..10 {
                let uri = format!("file:///project/pkg{i}.cabal");
                if let Some(doc) = map.get(&uri) {
                    // Source should always be parseable — never a torn read.
                    assert!(doc.source.contains("cabal-version:"));
                    assert!(doc.source.contains(&format!("name: pkg{i}")));
                }
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn rapid_open_close_cycle() {
    let docs = make_docs();

    for cycle in 0..100 {
        let uri = format!("file:///project/cycle{cycle}.cabal");
        {
            let mut map = docs.write().await;
            map.insert(
                uri.clone(),
                DocumentState::new(sample_cabal(&format!("cycle{cycle}")), 1),
            );
        }
        {
            let mut map = docs.write().await;
            map.remove(&uri);
        }
    }

    let map = docs.read().await;
    assert!(map.is_empty());
}

#[tokio::test]
async fn concurrent_open_close_different_documents() {
    let docs = make_docs();

    let mut handles = Vec::new();

    // Half the tasks open documents.
    for i in 0..50 {
        let docs = docs.clone();
        handles.push(tokio::spawn(async move {
            let uri = format!("file:///project/doc{i}.cabal");
            let mut map = docs.write().await;
            map.insert(uri, DocumentState::new(sample_cabal(&format!("doc{i}")), 1));
        }));
    }

    // Other half close different documents (no-ops if not yet opened).
    for i in 50..100 {
        let docs = docs.clone();
        handles.push(tokio::spawn(async move {
            let uri = format!("file:///project/doc{i}.cabal");
            let mut map = docs.write().await;
            map.remove(&uri);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // Should have exactly the 50 opened documents (the closes targeted different URIs).
    let map = docs.read().await;
    assert_eq!(map.len(), 50);
}

#[tokio::test]
async fn version_monotonicity_under_sequential_updates() {
    let docs = make_docs();
    let uri = "file:///project/test.cabal".to_string();

    {
        let mut map = docs.write().await;
        map.insert(uri.clone(), DocumentState::new(sample_cabal("test"), 1));
    }

    for version in 2..=100 {
        let mut map = docs.write().await;
        if let Some(doc) = map.get_mut(&uri) {
            doc.update(sample_cabal("test"), version);
        }
    }

    let map = docs.read().await;
    let doc = map.get(&uri).unwrap();
    assert_eq!(doc.version, 100);
}
