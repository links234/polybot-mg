use rocksdb::{DB, Options, IteratorMode, WriteBatch};
use crate::typed_store::{table::Table, codec::{CodecError, RocksDbKey, RocksDbValue}};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
}

/// Wraps a RocksDB instance and exposes typed put/get/scan operations.
/// 
/// Each table is distinguished by a prefix byte, allowing multiple logical
/// tables to coexist in a single RocksDB instance with type safety.
pub struct TypedStore {
    db: DB,
}

impl TypedStore {
    /// Open (or create) a RocksDB database at the specified path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(1000);
        opts.set_use_fsync(false);
        opts.set_bytes_per_sync(8388608);
        opts.optimize_for_point_lookup(1024);
        
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    /// Insert or update a key-value pair for the specified table.
    pub fn put<T: Table>(&self, key: &T::Key, value: &T::Value) -> Result<(), StoreError> {
        let mut k = vec![T::PREFIX];
        k.extend(key.encode_key());
        let v = value.encode_value()?;
        self.db.put(k, v)?;
        Ok(())
    }

    /// Retrieve a value by key for the specified table.
    /// Returns None if the key doesn't exist.
    pub fn get<T: Table>(&self, key: &T::Key) -> Result<Option<T::Value>, StoreError> {
        let mut k = vec![T::PREFIX];
        k.extend(key.encode_key());
        
        match self.db.get(k)? {
            Some(bytes) => Ok(Some(T::Value::decode_value(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Delete a key-value pair for the specified table.
    #[allow(dead_code)]
    pub fn delete<T: Table>(&self, key: &T::Key) -> Result<(), StoreError> {
        let mut k = vec![T::PREFIX];
        k.extend(key.encode_key());
        self.db.delete(k)?;
        Ok(())
    }

    /// Check if a key exists for the specified table.
    pub fn exists<T: Table>(&self, key: &T::Key) -> Result<bool, StoreError> {
        let mut k = vec![T::PREFIX];
        k.extend(key.encode_key());
        Ok(self.db.get(k)?.is_some())
    }

    /// Scan all key-value pairs for a table with prefix filtering.
    /// Returns an iterator over (Key, Value) pairs.
    pub fn scan<T: Table>(&self) -> Result<Vec<(T::Key, T::Value)>, StoreError> {
        let prefix = [T::PREFIX];
        let iter = self.db.iterator(IteratorMode::From(&prefix, rocksdb::Direction::Forward));
        
        let mut results = Vec::new();
        for item in iter {
            let (k, v) = item?;
            
            // Stop when we move past our prefix
            if !k.starts_with(&prefix) {
                break;
            }
            
            // Skip the prefix byte when decoding the key
            let key = T::Key::decode_key(&k[1..])?;
            let value = T::Value::decode_value(&v)?;
            results.push((key, value));
        }
        
        Ok(results)
    }

    /// Scan with a key prefix filter (useful for composite keys).
    #[allow(dead_code)]
    pub fn scan_prefix<T: Table>(&self, key_prefix: &[u8]) -> Result<Vec<(T::Key, T::Value)>, StoreError> {
        let mut full_prefix = vec![T::PREFIX];
        full_prefix.extend_from_slice(key_prefix);
        
        let iter = self.db.iterator(IteratorMode::From(&full_prefix, rocksdb::Direction::Forward));
        
        let mut results = Vec::new();
        for item in iter {
            let (k, v) = item?;
            
            // Stop when we move past our prefix
            if !k.starts_with(&full_prefix) {
                break;
            }
            
            // Skip the table prefix byte when decoding the key
            let key = T::Key::decode_key(&k[1..])?;
            let value = T::Value::decode_value(&v)?;
            results.push((key, value));
        }
        
        Ok(results)
    }

    /// Perform a batch write operation.
    pub fn batch_write<F>(&self, f: F) -> Result<(), StoreError>
    where
        F: FnOnce(&mut BatchWriter) -> Result<(), StoreError>,
    {
        let mut batch = WriteBatch::default();
        let mut writer = BatchWriter::new(&mut batch);
        f(&mut writer)?;
        self.db.write(batch)?;
        Ok(())
    }

    /// Get approximate count of keys for a table (fast but not exact).
    #[allow(dead_code)]
    pub fn approximate_count<T: Table>(&self) -> Result<u64, StoreError> {
        let prefix = [T::PREFIX];
        let mut count = 0u64;
        
        let iter = self.db.iterator(IteratorMode::From(&prefix, rocksdb::Direction::Forward));
        for item in iter {
            let (k, _) = item?;
            if !k.starts_with(&prefix) {
                break;
            }
            count += 1;
            
            // Sample for performance on large datasets
            if count % 10000 == 0 {
                // This gives us a rough estimate
                break;
            }
        }
        
        Ok(count)
    }
}

/// Helper for batch write operations.
pub struct BatchWriter<'a> {
    batch: &'a mut WriteBatch,
}

impl<'a> BatchWriter<'a> {
    fn new(batch: &'a mut WriteBatch) -> Self {
        Self { batch }
    }

    pub fn put<T: Table>(&mut self, key: &T::Key, value: &T::Value) -> Result<(), StoreError> {
        let mut k = vec![T::PREFIX];
        k.extend(key.encode_key());
        let v = value.encode_value()?;
        self.batch.put(k, v);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete<T: Table>(&mut self, key: &T::Key) -> Result<(), StoreError> {
        let mut k = vec![T::PREFIX];
        k.extend(key.encode_key());
        self.batch.delete(k);
        Ok(())
    }
}