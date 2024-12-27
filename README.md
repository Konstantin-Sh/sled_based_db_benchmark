# Sled-Based Storage Performance Analysis

This repository contains implementations and benchmarks for two variants of a Sled-based storage system: a vanilla implementation and a [cached implementation](https://github.com/hicaru/test-storage-tree/). The project aims to compare performance characteristics and memory usage.

## Overview
This project provides tools for analyzing the performance characteristics of two different Sled-based storage implementations:
- **Cached Storage**: An enhanced [implementation](https://github.com/hicaru/test-storage-tree/) with an in-memory cache layer
- **Vanilla Storage**: A basic implementation using Sled directly, it is cleared version of **Cached Storage**

## Benchmarks

The project includes comprehensive benchmarks comparing both implementations:

### Database Operations
1. **Database Creation**: Testing performance of creating a database with 1000 nodes
2. **Single Node Operations**:
   - Addition of a single node
   - Retrieval of a single node
   - Retrieval of non-existent node
3. **Concurrent Access**:
   - Multiple threads (10, 50, 100 threads)
   - Mixed workload (add/get/update operations)
   - Real-world simulation with random delays
4. **Memory Usage**

### Running Benchmarks

The project includes several benchmark categories:

#### Performance Benchmarks
```bash
# Run all benchmarks
cargo bench -p storage-benchmarks

# Run specific benchmark group
cargo bench -p storage-benchmarks -- create_db
cargo bench -p storage-benchmarks -- bench_add_node
cargo bench -p storage-benchmarks -- bench_get_node
cargo bench -p storage-benchmarks -- get_nonexistent_node
cargo bench -p storage-benchmarks -- concurrent_access
```
#### Memory Usage Analysis
```bash
# Run memory usage comparison
cargo run --release -p storage-benchmarks
```



## Installation

### Prerequisites
- Rust
- Cargo package manager
- Linux/Unix environment (for jemalloc support, jemalloc allocator is used for memory profiling)


### Setup
```bash
# Clone the repository
git clone https://github.com/Konstantin-Sh/sled_based_db_benchmark.git
cd sled-based-db-benchmark

# Build the project
cargo build --release
```

## Example Output

### Memory Usage Analysis
```
Starting memory usage comparison...
...
Run tests

Measuring Vanilla Storage - add:
Baseline memory usage: 22722952 bytes
Final memory usage: 29649512 bytes
Memory difference: 6926560 bytes

Measuring Cached Storage - add:
Baseline memory usage: 22747128 bytes
Final memory usage: 29672440 bytes
Memory difference: 6925312 bytes
```

### Performance Benchmark
```
get_nonexistent_node/cached_storage
                        time:   [18.176 µs 24.075 µs 32.104 µs]
...
get_nonexistent_node/vanilla_storage
                        time:   [8.2196 µs 12.008 µs 17.081 µs]
...
Cached Storage - Threads: 100, Avg latency: 57.042µs, Throughput: 17530.87 ops/sec
concurrent_access/cached_storage/threads_100_ops_100
                        time:   [692.93 ms 724.06 ms 762.25 ms]
...
Vanilla Storage - Threads: 100, Avg latency: 31.728µs, Throughput: 31517.59 ops/sec
concurrent_access/vanilla_storage/threads_100_ops_100
                        time:   [660.68 ms 666.04 ms 671.50 ms]
```


## Project Structure

```
sled-based-db-benchmark/
├── test-storage-tree/     # Cached [implementation](https://github.com/hicaru/test-storage-tree/)
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── vanilla-storage-tree/  # Basic implementation
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── storage-benchmarks/    # Benchmark suite
│   ├── benches/           # Criterion preformance benchmarks
│   │   └── storage_benchmarks.rs
|   ├── src/               # Memory usage analysis
│   │   └── main.rs
│   └── Cargo.toml
└── Cargo.toml            # Workspace configuration
```

## Testing

The project includes test coverage for both storage implementations:

```bash
# Run all tests
cargo test --workspace

# Run specific tests
cargo test -p concurrent_binary
cargo test -p vanilla_sled
```

### Test Categories
1. **Basic Operations**: CRUD operation testing
2. **Multiple Nodes**: Testing with large numbers of nodes
3. **Persistence**: Data durability testing
4. **Concurrent Access**: Thread safety and race condition testing

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the GPL-3.0 License - see the LICENSE file for details.

## Acknowledgments

- Sled database
- Tokio async runtime
- Criterion benchmarking framework
- Jemalloc
- Claude.ai
