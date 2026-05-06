use aura_core::storage::{StorageEngine, StorageRequest};
use aura_core::worker::Segment;
use aura_core::TaskId;
use bytes::Bytes;
use criterion::{criterion_group, criterion_main, Criterion};
use std::path::PathBuf;
use tempfile::tempdir;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

fn bench_storage_sequential_write(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("bench_seq.bin");

    let (request_tx, request_rx) = mpsc::channel(100);
    let (completion_tx, _completion_rx) = mpsc::channel(100);
    let mut storage = StorageEngine::new(request_rx, completion_tx);
    let id = TaskId(1);
    storage.register_task(id, file_path);

    rt.spawn(async move {
        storage.run().await.unwrap();
    });

    let data = Bytes::from(vec![0u8; 1024 * 1024]); // 1MB chunk
    let mut offset = 0;

    c.bench_function("storage_sequential_write_1mb", |b| {
        b.to_async(&rt).iter(|| {
            let req = StorageRequest::Write {
                task_id: id,
                segment: Segment {
                    offset,
                    length: 1024 * 1024,
                },
                data: data.clone(),
            };
            offset += 1024 * 1024;
            let tx = request_tx.clone();
            async move {
                tx.send(req).await.unwrap();
            }
        })
    });
}

fn bench_storage_random_write_aggregated(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("bench_rand.bin");

    let (request_tx, request_rx) = mpsc::channel(1000);
    let (completion_tx, _completion_rx) = mpsc::channel(1000);
    let mut storage = StorageEngine::new(request_rx, completion_tx);
    let id = TaskId(2);
    storage.register_task(id, file_path);

    rt.spawn(async move {
        storage.run().await.unwrap();
    });

    let data = Bytes::from(vec![0u8; 16 * 1024]); // 16KB chunk (typical for BT)
    
    // We'll send them out of order to trigger aggregation
    let mut offsets: Vec<u64> = (0..100).map(|i| i * 16 * 1024).collect();
    
    let mut i = 0;
    c.bench_function("storage_random_write_16kb", |b| {
        b.to_async(&rt).iter(|| {
            let offset = if i % 2 == 0 {
                offsets[i % 100] + 16 * 1024 // Send "next" first
            } else {
                offsets[i % 100] // Send "current" second (triggering flush)
            };
            
            let req = StorageRequest::Write {
                task_id: id,
                segment: Segment {
                    offset,
                    length: 16 * 1024,
                },
                data: data.clone(),
            };
            i += 1;
            let tx = request_tx.clone();
            async move {
                tx.send(req).await.unwrap();
            }
        })
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_storage_sequential_write, bench_storage_random_write_aggregated
);
criterion_main!(benches);
