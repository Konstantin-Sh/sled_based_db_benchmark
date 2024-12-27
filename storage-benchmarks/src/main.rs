use concurrent_binary::StorageManager as CachedStorage;
use jemallocator::Jemalloc;
use rand::Rng;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::sleep;
use vanilla_sled::StorageManager as VanillaStorage;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

// Helper function to get current memory usage
fn get_memory_usage() -> usize {
    let epoch = jemalloc_ctl::epoch::mib().unwrap();
    epoch.advance().unwrap();

    let allocated = jemalloc_ctl::stats::allocated::mib().unwrap();
    allocated.read().unwrap()
}

// Helper function to generate test data
fn generate_test_data(size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut data = vec![0u8; size];
    rng.fill(&mut data[..]);
    data
}

async fn measure_memory_vanilla(operation: &str) {
    println!("\nMeasuring Vanilla Storage - {operation}:");

    // Create temporary directory
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("bench_db");

    // Baseline memory
    let baseline_memory = get_memory_usage();
    println!("Baseline memory usage: {baseline_memory} bytes");

    // Initialize storage
    let storage = VanillaStorage::new(db_path.to_str().unwrap(), 1).unwrap();

    // Generate test data
    let data = generate_test_data(1024); // 1KB of data
    let hash = blake3::hash(&data).into();

    // Measure operation
    match operation {
        "add" => {
            storage.add_node(data, hash).await.unwrap();
        }
        "delete" => {
            storage.add_node(data.clone(), hash).await.unwrap();
            sleep(Duration::from_millis(100)).await; // Wait for operation to complete
            storage.delete_node(&hash).await.unwrap();
        }
        _ => panic!("Unknown operation"),
    }

    let final_memory = get_memory_usage();
    println!("Final memory usage: {final_memory} bytes");
    println!(
        "Memory difference: {} bytes",
        final_memory as i128 - baseline_memory as i128
    );
}

async fn measure_memory_cached(operation: &str) {
    println!("\nMeasuring Cached Storage - {operation}:");

    // Create temporary directory
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("bench_db");

    // Baseline memory
    let baseline_memory = get_memory_usage();
    println!("Baseline memory usage: {baseline_memory} bytes");

    // Initialize storage
    let storage = CachedStorage::new(db_path.to_str().unwrap(), 1).unwrap();

    // Generate test data
    let data = generate_test_data(1024); // 1KB of data
    let hash = blake3::hash(&data).into();

    // Measure operation
    match operation {
        "add" => {
            storage.add_node(data, hash).await.unwrap();
        }
        "delete" => {
            storage.add_node(data.clone(), hash).await.unwrap();
            sleep(Duration::from_millis(100)).await; // Wait for operation to complete
            storage.delete_node(&hash).await.unwrap();
        }
        _ => panic!("Unknown operation"),
    }

    let final_memory = get_memory_usage();
    println!("Final memory usage: {final_memory} bytes");
    println!(
        "Memory difference: {} bytes",
        final_memory as i128 - baseline_memory as i128
    );
}

#[tokio::main]
async fn main() {
    println!("Starting memory usage comparison...");

    println!("\nWarm up");
    measure_memory_cached("add").await;
    println!("\nRun tests");
    // Measure add operations
    measure_memory_vanilla("add").await;
    measure_memory_cached("add").await;

    // Small delay between tests
    sleep(Duration::from_secs(1)).await;

    // Measure delete operations
    measure_memory_vanilla("delete").await;
    measure_memory_cached("delete").await;
}
