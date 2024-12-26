use concurrent_binary::{Node, StorageError};
use sled::Db;

/// Type alias for Result with `StorageError` as error type
type Result<T> = std::result::Result<T, StorageError>;

/// Main storage manager that handles persistent storage only
pub struct StorageManager {
    /// Persistent storage using sled database
    pub db: Db,
}

impl StorageManager {
    /// Creates a new `StorageManager` instance
    ///
    /// # Arguments
    /// * `db_path` - Path to the sled database file
    /// * `_number_of_nodes` - (unused) Number of nodes for memory pre-allocation
    ///                       Will allocate space for `number_of_nodes` + 1 to optimize insertions
    ///
    /// # Returns
    /// * `Result<Self>` - New `StorageManager` instance or error
    pub fn new(db_path: &str, _number_of_nodes: usize) -> Result<Self> {
        let db = sled::open(db_path)?;
        Ok(Self { db })
    }

    /// Adds a new node to persistent storage
    ///
    /// # Arguments
    /// * `cipher_data` - Encrypted data to store
    /// * `hash` - BLAKE3 hash of the data
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn add_node(&self, cipher_data: Vec<u8>, hash: [u8; 32]) -> Result<()> {
        let size = cipher_data.len();
        let node = Node {
            hash,
            size,
            cipher_data: Some(cipher_data),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        self.db.insert(hash, bincode::serialize(&node)?)?;

        Ok(())
    }

    /// Retrieves a node by its hash
    ///
    /// # Arguments
    /// * `hash` - BLAKE3 hash of the node to retrieve
    ///
    /// # Returns
    /// * `Result<Option<Node>>` - Node if found, None if not found, or error
    pub async fn get_node(&self, hash: &[u8; 32]) -> Result<Option<Node>> {
        if let Some(data) = self.db.get(hash)? {
            Ok(Some(bincode::deserialize(&data)?))
        } else {
            Ok(None)
        }
    }

    /// Deletes a node from storage
    ///
    /// # Arguments
    /// * `hash` - BLAKE3 hash of the node to delete
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn delete_node(&self, hash: &[u8; 32]) -> Result<()> {
        self.db.remove(hash)?;

        Ok(())
    }

    /// Updates an existing node with new data
    ///
    /// # Arguments
    /// * `hash` - BLAKE3 hash of the node to update
    /// * `new_cipher_data` - New encrypted data
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn update_node(&self, hash: &[u8; 32], new_cipher_data: Vec<u8>) -> Result<()> {
        // Implement update as delete + add for simplicity and consistency
        self.delete_node(hash).await?;
        self.add_node(new_cipher_data, *hash).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Generates random encrypted data of fixed size (1024 bytes)
    /// This simulates encrypted records that would be stored in the system
    ///
    /// # Returns
    /// * `Vec<u8>` - Vector containing random bytes to simulate encrypted data
    fn generate_cipher_data() -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut data = vec![0u8; 1024]; // Fixed size to simulate real-world encrypted records
        rng.fill(&mut data[..]);
        data
    }

    /// Test basic CRUD (Create, Read, Update, Delete) operations
    /// Verifies that:
    /// - A node can be added to storage
    /// - A node can be retrieved with correct data
    /// - A node can be updated with new data
    /// - A node can be deleted
    #[tokio::test]
    async fn test_storage_basic_operations() -> Result<()> {
        // Setup temporary test directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_db");

        // Initialize storage with moderate cache size
        let storage = StorageManager::new(db_path.to_str().unwrap(), 400)?;

        // Test node creation and storage
        let cipher_data = generate_cipher_data();
        let hash = blake3::hash(&cipher_data).into();
        storage.add_node(cipher_data.clone(), hash).await?;

        // Verify node retrieval and data integrity
        let retrieved_node = storage.get_node(&hash).await?.unwrap();
        assert_eq!(retrieved_node.cipher_data.unwrap(), cipher_data);
        assert_eq!(retrieved_node.size, 1024);

        // Test node update functionality
        let new_cipher_data = generate_cipher_data();
        storage.update_node(&hash, new_cipher_data.clone()).await?;
        let updated_node = storage.get_node(&hash).await?.unwrap();
        assert_eq!(updated_node.cipher_data.unwrap(), new_cipher_data);

        // Test node deletion
        storage.delete_node(&hash).await?;
        assert!(storage.get_node(&hash).await?.is_none());

        Ok(())
    }

    /// Tests handling of multiple nodes
    /// Verifies that:
    /// - Multiple nodes can be stored successfully
    /// - Each node maintains data integrity
    /// - The system can handle the specified amount of nodes
    #[tokio::test]
    async fn test_storage_multiple_nodes() -> Result<()> {
        const AMOUNT_OF_NODES: usize = 400;

        // Setup temporary test environment
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_db_multiple");

        // Initialize storage with capacity matching test size
        let storage = StorageManager::new(db_path.to_str().unwrap(), AMOUNT_OF_NODES)?;
        let mut hashes = Vec::new();

        // Create and store multiple nodes
        for _ in 0..AMOUNT_OF_NODES {
            let cipher_data = generate_cipher_data();
            let hash = blake3::hash(&cipher_data).into();
            storage.add_node(cipher_data.clone(), hash).await?;
            hashes.push((hash, cipher_data));
        }

        // Verify each node's integrity
        for (hash, original_data) in hashes {
            let node = storage.get_node(&hash).await?.unwrap();
            assert_eq!(node.cipher_data.unwrap(), original_data);
            assert_eq!(node.size, 1024);
        }

        Ok(())
    }

    /// Tests the cache size limitation and data persistence
    /// Verifies that:
    /// - Nodes are properly persisted in database
    /// - Nodes can be retrieved even if not in cache
    #[tokio::test]
    async fn test_storage_persistence() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_db_persistence");

        // Create multiple nodes
        let storage = StorageManager::new(db_path.to_str().unwrap(), 5)?;
        let mut hashes = Vec::new();

        // Add 10 nodes with small cache size
        for _ in 0..10 {
            let cipher_data = generate_cipher_data();
            let hash = blake3::hash(&cipher_data).into();
            storage.add_node(cipher_data, hash).await?;
            hashes.push(hash);
        }

        // Verify all nodes are still accessible
        for hash in hashes {
            let node = storage.get_node(&hash).await?;
            assert!(node.is_some());
            assert_eq!(node.unwrap().size, 1024);
        }

        Ok(())
    }

    /// Tests concurrent access to the storage system
    /// Simulates real-world scenario with multiple threads performing
    /// different operations simultaneously
    #[tokio::test]
    async fn test_concurrent_access() -> Result<()> {
        use futures::future::join_all;
        use rand::seq::SliceRandom;
        use std::time::Duration;
        use tokio::time::sleep;

        const THREADS: usize = 100;
        const OPERATIONS_PER_THREAD: usize = 50;

        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_db_concurrent");
        let storage = Arc::new(StorageManager::new(db_path.to_str().unwrap(), 1000)?);

        // Initialize with some data
        let mut initial_hashes = Vec::new();
        for _ in 0..20 {
            let data = generate_cipher_data();
            let hash = blake3::hash(&data).into();
            storage.add_node(data, hash).await?;
            initial_hashes.push(hash);
        }

        let mut handles = Vec::new();
        let initial_hashes = Arc::new(initial_hashes);

        // Spawn multiple threads
        for thread_id in 0..THREADS {
            let storage = storage.clone();
            let hashes = initial_hashes.clone();

            let handle = tokio::spawn(async move {
                for op_id in 0..OPERATIONS_PER_THREAD {
                    let op = {
                        let mut rng = rand::thread_rng();
                        let delay = rng.gen_range(0..10);
                        let op_type = rng.gen_range(0..3); // Reduced to 3 operations
                        let selected_hash = hashes.choose(&mut rng).cloned();
                        let new_data = if op_type == 0 || op_type == 2 {
                            Some(generate_cipher_data())
                        } else {
                            None
                        };
                        (delay, op_type, selected_hash, new_data)
                    };

                    sleep(Duration::from_millis(op.0)).await;

                    match op.1 {
                        0 => {
                            // Add new node
                            if let Some(data) = op.3 {
                                let hash = blake3::hash(&data).into();
                                if let Err(e) = storage.add_node(data, hash).await {
                                    eprintln!("Thread {} op {} add error: {}", thread_id, op_id, e);
                                }
                            }
                        }
                        1 => {
                            // Read node
                            if let Some(hash) = op.2 {
                                if let Err(e) = storage.get_node(&hash).await {
                                    eprintln!(
                                        "Thread {} op {} read error: {}",
                                        thread_id, op_id, e
                                    );
                                }
                            }
                        }
                        2 => {
                            // Update node
                            if let Some(hash) = op.2 {
                                if let Some(new_data) = op.3 {
                                    if let Err(e) = storage.update_node(&hash, new_data).await {
                                        eprintln!(
                                            "Thread {} op {} update error: {}",
                                            thread_id, op_id, e
                                        );
                                    }
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        let results = join_all(handles).await;
        for result in results {
            result.expect("Task panicked");
        }

        // Verify data integrity by checking initial nodes
        for hash in initial_hashes.iter() {
            let node = storage.get_node(hash).await?;
            assert!(node.is_some(), "Initial node should still be accessible");
        }

        Ok(())
    }
}
