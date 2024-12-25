// storage-benchmarks/benches/storage_benchmarks.rs
use concurrent_binary::StorageManager as CachedStorage;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::Rng;
use std::time::Duration;
use tempfile::tempdir;
use vanilla_sled::StorageManager as VanillaStorage;

// Helper function to generate consistent test data
fn generate_test_data(size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut data = vec![0u8; size];
    rng.fill(&mut data[..]);
    data
}

// Benchmark database creation with 1000 nodes
fn bench_create_db(c: &mut Criterion) {
    let mut group = c.benchmark_group("create_db_1000_nodes");
    group.sample_size(10); // Reduced sample size due to operation cost
    group.measurement_time(Duration::from_secs(60));

    let data_size = 1024; // 1KB per node

    group.bench_function("cached_storage", |b| {
        b.iter_batched(
            || {
                // Setup: Create temp directory
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                (temp_dir, db_path)
            },
            |(temp_dir, db_path)| {
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    let storage = CachedStorage::new(db_path.to_str().unwrap(), 1000).unwrap();

                    // Create 1000 nodes
                    for _ in 0..1000 {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                    }
                });
                temp_dir // Return temp_dir to ensure it's not dropped early
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("vanilla_storage", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                (temp_dir, db_path)
            },
            |(temp_dir, db_path)| {
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    let storage = VanillaStorage::new(db_path.to_str().unwrap(), 1000).unwrap();

                    for _ in 0..1000 {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                    }
                });
                temp_dir
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

// Benchmark single node addition
fn bench_add_node(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_single_node");

    let data_size = 1024;
    let setup_nodes = 1000;

    group.bench_function("cached_storage", |b| {
        b.iter_batched(
            || {
                // Setup: Create DB with 1000 nodes
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let storage = runtime.block_on(async {
                    let storage =
                        CachedStorage::new(db_path.to_str().unwrap(), setup_nodes).unwrap();
                    // Pre-populate with setup_nodes
                    for _ in 0..setup_nodes {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                    }
                    storage
                });
                (temp_dir, storage, runtime)
            },
            |(temp_dir, storage, runtime)| {
                runtime.block_on(async {
                    let data = generate_test_data(data_size);
                    let hash = blake3::hash(&data).into();
                    storage.add_node(data, hash).await.unwrap();
                });
                (temp_dir, storage, runtime)
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("vanilla_storage", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let storage = runtime.block_on(async {
                    let storage =
                        VanillaStorage::new(db_path.to_str().unwrap(), setup_nodes).unwrap();
                    for _ in 0..setup_nodes {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                    }
                    storage
                });
                (temp_dir, storage, runtime)
            },
            |(temp_dir, storage, runtime)| {
                runtime.block_on(async {
                    let data = generate_test_data(data_size);
                    let hash = blake3::hash(&data).into();
                    storage.add_node(data, hash).await.unwrap();
                });
                (temp_dir, storage, runtime)
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

// Benchmark node retrieval
fn bench_get_node(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_single_node");

    let data_size = 1024;
    let setup_nodes = 1000;

    group.bench_function("cached_storage", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let (storage, target_hash) = runtime.block_on(async {
                    let storage =
                        CachedStorage::new(db_path.to_str().unwrap(), setup_nodes).unwrap();
                    let mut target_hash = None;

                    // Pre-populate and save one hash for testing
                    for i in 0..setup_nodes {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                        if i == setup_nodes / 2 {
                            target_hash = Some(hash);
                        }
                    }
                    (storage, target_hash.unwrap())
                });
                (temp_dir, storage, target_hash, runtime)
            },
            |(temp_dir, storage, target_hash, runtime)| {
                runtime.block_on(async {
                    storage.get_node(&target_hash).await.unwrap();
                });
                (temp_dir, storage, target_hash, runtime)
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("vanilla_storage", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let (storage, target_hash) = runtime.block_on(async {
                    let storage =
                        VanillaStorage::new(db_path.to_str().unwrap(), setup_nodes).unwrap();
                    let mut target_hash = None;

                    for i in 0..setup_nodes {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                        if i == setup_nodes / 2 {
                            target_hash = Some(hash);
                        }
                    }
                    (storage, target_hash.unwrap())
                });
                (temp_dir, storage, target_hash, runtime)
            },
            |(temp_dir, storage, target_hash, runtime)| {
                runtime.block_on(async {
                    storage.get_node(&target_hash).await.unwrap();
                });
                (temp_dir, storage, target_hash, runtime)
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(benches, bench_create_db, bench_add_node, bench_get_node);
criterion_main!(benches);
