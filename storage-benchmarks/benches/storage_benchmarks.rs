// storage-benchmarks/benches/storage_benchmarks.rs
use concurrent_binary::StorageManager as CachedStorage;
use concurrent_binary::{Node, StorageError};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::seq::SliceRandom;
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::sleep;
use vanilla_sled::StorageManager as VanillaStorage;

/// Enum to represent different storage operations
#[derive(Debug, Clone)]
enum StorageOp {
    Add,
    Get,
    Update,
}

/// Represents configuration for concurrent benchmark
#[derive(Debug, Copy, Clone)]
struct ConcurrentBenchConfig {
    thread_count: usize,
    operations_per_thread: usize,
    initial_nodes: usize,
    data_size: usize,
}

/// Define a trait for storage operations to enforce required methods
#[async_trait::async_trait]
trait StorageOps {
    async fn add_node(&self, data: Vec<u8>, hash: [u8; 32]) -> Result<(), StorageError>;
    async fn get_node(&self, hash: &[u8; 32]) -> Result<Option<Node>, StorageError>;
    async fn update_node(&self, hash: &[u8; 32], data: Vec<u8>) -> Result<(), StorageError>;
}

#[async_trait::async_trait]
impl StorageOps for CachedStorage {
    async fn add_node(&self, data: Vec<u8>, hash: [u8; 32]) -> Result<(), StorageError> {
        self.add_node(data, hash).await
    }

    async fn get_node(&self, hash: &[u8; 32]) -> Result<Option<Node>, StorageError> {
        self.get_node(hash).await
    }

    async fn update_node(&self, hash: &[u8; 32], data: Vec<u8>) -> Result<(), StorageError> {
        self.update_node(hash, data).await
    }
}

#[async_trait::async_trait]
impl StorageOps for VanillaStorage {
    async fn add_node(&self, data: Vec<u8>, hash: [u8; 32]) -> Result<(), StorageError> {
        self.add_node(data, hash).await
    }

    async fn get_node(&self, hash: &[u8; 32]) -> Result<Option<Node>, StorageError> {
        self.get_node(hash).await
    }

    async fn update_node(&self, hash: &[u8; 32], data: Vec<u8>) -> Result<(), StorageError> {
        self.update_node(hash, data).await
    }
}

async fn run_concurrent_ops<T>(
    storage: Arc<T>,
    config: ConcurrentBenchConfig,
    initial_hashes: Arc<Vec<[u8; 32]>>,
) -> Vec<Duration>
where
    T: StorageOps + Send + Sync + 'static,
{
    let mut handles = Vec::new();
    let mut durations = Vec::new();

    for _thread_id in 0..config.thread_count {
        let storage = storage.clone();
        let hashes = initial_hashes.clone();

        let handle = tokio::spawn(async move {
            let mut thread_durations = Vec::new();
            //            let mut rng = rand::thread_rng();

            for _ in 0..config.operations_per_thread {
                let op = {
                    let mut rng = rand::thread_rng();
                    match rng.gen_range(0..3) {
                        0 => StorageOp::Add,
                        1 => StorageOp::Get,
                        _ => StorageOp::Update,
                    }
                };

                let start = std::time::Instant::now();

                match op {
                    StorageOp::Add => {
                        let data = generate_test_data(config.data_size);
                        let hash = blake3::hash(&data).into();
                        let _ = storage.add_node(data, hash).await;
                    }
                    StorageOp::Get => {
                        if let Some(hash) = {
                            let mut rng = rand::thread_rng();
                            hashes.choose(&mut rng).cloned()
                        } {
                            let _ = storage.get_node(&hash).await;
                        }
                    }
                    StorageOp::Update => {
                        if let Some(hash) = {
                            let mut rng = rand::thread_rng();
                            hashes.choose(&mut rng).cloned()
                        } {
                            let new_data = generate_test_data(config.data_size);
                            let _ = storage.update_node(&hash, new_data).await;
                        }
                    }
                }

                thread_durations.push(start.elapsed());

                // Generate delay with new thread_rng instance
                let delay = {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(0..10)
                };

                sleep(Duration::from_millis(delay)).await;
            }
            thread_durations
        });

        handles.push(handle);
    }

    for handle in handles {
        if let Ok(thread_durations) = handle.await {
            durations.extend(thread_durations);
        }
    }

    durations
}

fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    group.sample_size(10); // Reduced due to complexity of operations
    group.measurement_time(Duration::from_secs(30));

    // Test configurations
    let configs = vec![
        ConcurrentBenchConfig {
            thread_count: 10,
            operations_per_thread: 100,
            initial_nodes: 1000,
            data_size: 1024,
        },
        ConcurrentBenchConfig {
            thread_count: 50,
            operations_per_thread: 100,
            initial_nodes: 1000,
            data_size: 1024,
        },
        ConcurrentBenchConfig {
            thread_count: 100,
            operations_per_thread: 100,
            initial_nodes: 1000,
            data_size: 1024,
        },
    ];

    for config in configs {
        let benchmark_name = format!(
            "threads_{}_ops_{}",
            config.thread_count, config.operations_per_thread
        );

        // Benchmark cached implementation
        group.bench_with_input(
            BenchmarkId::new("cached_storage", benchmark_name.clone()),
            &config,
            |b, config| {
                b.iter_batched(
                    || {
                        let temp_dir = tempdir().unwrap();
                        let db_path = temp_dir.path().join("bench_db");
                        let runtime = tokio::runtime::Runtime::new().unwrap();

                        // Setup initial data
                        let (storage, initial_hashes) = runtime.block_on(async {
                            let storage = Arc::new(
                                CachedStorage::new(
                                    db_path.to_str().unwrap(),
                                    config.initial_nodes
                                ).unwrap()
                            );
                            let mut hashes = Vec::new();

                            // Create initial nodes
                            for _ in 0..config.initial_nodes {
                                let data = generate_test_data(config.data_size);
                                let hash = blake3::hash(&data).into();
                                storage.add_node(data, hash).await.unwrap();
                                hashes.push(hash);
                            }

                            (storage, Arc::new(hashes))
                        });

                        (temp_dir, storage, initial_hashes, runtime)
                    },
                    |(temp_dir, storage, initial_hashes, runtime)| {
                        runtime.block_on(async {
                            let durations = run_concurrent_ops(
                                storage.clone(),
                                *config,
                                initial_hashes.clone()
                            ).await;

                            // Calculate statistics
                            let total_ops = config.thread_count * config.operations_per_thread;
                            let total_duration: Duration = durations.iter().sum();
                            let avg_duration = total_duration / total_ops as u32;
                            let throughput = total_ops as f64 / total_duration.as_secs_f64();

                            println!(
                                "Cached Storage - Threads: {}, Avg latency: {:?}, Throughput: {:.2} ops/sec",
                                config.thread_count,
                                avg_duration,
                                throughput
                            );
                        });
                        (temp_dir, storage, initial_hashes, runtime)
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        // Benchmark vanilla implementation
        group.bench_with_input(
            BenchmarkId::new("vanilla_storage", benchmark_name.clone()),
            &config,
            |b, config| {
                b.iter_batched(
                    || {
                        let temp_dir = tempdir().unwrap();
                        let db_path = temp_dir.path().join("bench_db");
                        let runtime = tokio::runtime::Runtime::new().unwrap();

                        let (storage, initial_hashes) = runtime.block_on(async {
                            let storage = Arc::new(
                                VanillaStorage::new(
                                    db_path.to_str().unwrap(),
                                    config.initial_nodes
                                ).unwrap()
                            );
                            let mut hashes = Vec::new();

                            for _ in 0..config.initial_nodes {
                                let data = generate_test_data(config.data_size);
                                let hash = blake3::hash(&data).into();
                                storage.add_node(data, hash).await.unwrap();
                                hashes.push(hash);
                            }

                            (storage, Arc::new(hashes))
                        });

                        (temp_dir, storage, initial_hashes, runtime)
                    },
                    |(temp_dir, storage, initial_hashes, runtime)| {
                        runtime.block_on(async {
                            let durations = run_concurrent_ops(
                                storage.clone(),
                                *config,
                                initial_hashes.clone()
                            ).await;

                            let total_ops = config.thread_count * config.operations_per_thread;
                            let total_duration: Duration = durations.iter().sum();
                            let avg_duration = total_duration / total_ops as u32;
                            let throughput = total_ops as f64 / total_duration.as_secs_f64();

                            println!(
                                "Vanilla Storage - Threads: {}, Avg latency: {:?}, Throughput: {:.2} ops/sec",
                                config.thread_count,
                                avg_duration,
                                throughput
                            );
                        });
                        (temp_dir, storage, initial_hashes, runtime)
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

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

    group.measurement_time(Duration::from_secs(30));
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

    group.measurement_time(Duration::from_secs(20));
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

// Benchmark nonexistent node retrieval
fn bench_get_nonexistent_node(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_nonexistent_node");

    group.measurement_time(Duration::from_secs(30));
    let data_size = 1024;
    let setup_nodes = 1000;

    // Generate a hash that definitely won't exist in the database
    let nonexistent_data = generate_test_data(data_size);
    let nonexistent_hash = blake3::hash(&nonexistent_data).into();

    group.bench_function("cached_storage", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                let runtime = tokio::runtime::Runtime::new().unwrap();

                // Setup database with known data
                let storage = runtime.block_on(async {
                    let storage =
                        CachedStorage::new(db_path.to_str().unwrap(), setup_nodes).unwrap();

                    // Populate with data that's different from our test hash
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
                    // Try to retrieve the nonexistent node
                    let result = storage.get_node(&nonexistent_hash).await.unwrap();
                    assert!(result.is_none(), "Node should not exist");
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

                    // Populate with data that's different from our test hash
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
                    // Try to retrieve the nonexistent node
                    let result = storage.get_node(&nonexistent_hash).await.unwrap();
                    assert!(result.is_none(), "Node should not exist");
                });
                (temp_dir, storage, runtime)
            },
            criterion::BatchSize::LargeInput,
        )
    });

    // Add additional benchmark for cached implementation with "warm" cache
    group.bench_function("cached_storage_warm", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempdir().unwrap();
                let db_path = temp_dir.path().join("bench_db");
                let runtime = tokio::runtime::Runtime::new().unwrap();

                let storage = runtime.block_on(async {
                    let storage =
                        CachedStorage::new(db_path.to_str().unwrap(), setup_nodes).unwrap();

                    // Populate with data
                    for _ in 0..setup_nodes {
                        let data = generate_test_data(data_size);
                        let hash = blake3::hash(&data).into();
                        storage.add_node(data, hash).await.unwrap();
                    }

                    // Warm up the cache by doing some reads
                    for _ in 0..10 {
                        let _ = storage.get_node(&nonexistent_hash).await.unwrap();
                    }

                    storage
                });

                (temp_dir, storage, runtime)
            },
            |(temp_dir, storage, runtime)| {
                runtime.block_on(async {
                    let result = storage.get_node(&nonexistent_hash).await.unwrap();
                    assert!(result.is_none(), "Node should not exist");
                });
                (temp_dir, storage, runtime)
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_create_db,
    bench_add_node,
    bench_get_node,
    bench_get_nonexistent_node,
    bench_concurrent_access,
);
criterion_main!(benches);
