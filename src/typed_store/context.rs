//! TypedDbContext implementation for type-safe RocksDB operations
//! 
//! Provides type-safe database operations with column families and codecs

use rocksdb::{DB, Options, ColumnFamilyDescriptor, WriteBatch, IteratorMode};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

use crate::typed_store::{
    table::TypedCf,
    codec::{DbCodec, CodecError}
};

#[derive(Debug, Error)]
pub enum DbContextError {
    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),
}

/// Database context providing type-safe operations with column families
pub struct TypedDbContext {
    db: Arc<DB>,
}

impl TypedDbContext {
    /// Open database with specified column families
    pub fn open<P: AsRef<Path>>(
        path: P,
        column_families: Vec<&'static str>,
    ) -> Result<Self, DbContextError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(1000);
        opts.set_use_fsync(false);
        opts.set_bytes_per_sync(8388608);
        opts.optimize_for_point_lookup(1024);
        
        // Create column family descriptors
        let cf_descriptors: Vec<ColumnFamilyDescriptor> = column_families
            .iter()
            .map(|&name| {
                let mut cf_opts = Options::default();
                cf_opts.optimize_for_point_lookup(1024);
                ColumnFamilyDescriptor::new(name, cf_opts)
            })
            .collect();
        
        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
        
        Ok(Self {
            db: Arc::new(db),
        })
    }
    
    /// Put a key-value pair in the specified column family
    #[allow(dead_code)]
    pub fn put<CF: TypedCf>(&self, key: &CF::Key, value: &CF::Value) -> Result<(), DbContextError> {
        let cf = self.get_cf_handle::<CF>()?;
        let key_bytes = CF::KeyCodec::encode(key)?;
        let value_bytes = CF::ValueCodec::encode(value)?;
        
        self.db.put_cf(&cf, key_bytes, value_bytes)?;
        Ok(())
    }
    
    /// Get a value by key from the specified column family
    pub fn get<CF: TypedCf>(&self, key: &CF::Key) -> Result<Option<CF::Value>, DbContextError> {
        let cf = self.get_cf_handle::<CF>()?;
        let key_bytes = CF::KeyCodec::encode(key)?;
        
        match self.db.get_cf(&cf, key_bytes)? {
            Some(value_bytes) => {
                let value = CF::ValueCodec::decode(&value_bytes)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
    
    /// Delete a key from the specified column family
    #[allow(dead_code)]
    pub fn delete<CF: TypedCf>(&self, key: &CF::Key) -> Result<(), DbContextError> {
        let cf = self.get_cf_handle::<CF>()?;
        let key_bytes = CF::KeyCodec::encode(key)?;
        
        self.db.delete_cf(&cf, key_bytes)?;
        Ok(())
    }
    
    /// Check if a key exists in the specified column family
    pub fn exists<CF: TypedCf>(&self, key: &CF::Key) -> Result<bool, DbContextError> {
        let cf = self.get_cf_handle::<CF>()?;
        let key_bytes = CF::KeyCodec::encode(key)?;
        
        Ok(self.db.get_cf(&cf, key_bytes)?.is_some())
    }
    
    /// Scan all key-value pairs in the specified column family
    pub fn scan<CF: TypedCf>(&self) -> Result<Vec<(CF::Key, CF::Value)>, DbContextError> {
        let cf = self.get_cf_handle::<CF>()?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        
        let mut results = Vec::new();
        for item in iter {
            let (key_bytes, value_bytes) = item?;
            let key = CF::KeyCodec::decode(&key_bytes)?;
            let value = CF::ValueCodec::decode(&value_bytes)?;
            results.push((key, value));
        }
        
        Ok(results)
    }
    
    /// Scan with key prefix in the specified column family
    #[allow(dead_code)]
    pub fn scan_prefix<CF: TypedCf>(&self, prefix: &[u8]) -> Result<Vec<(CF::Key, CF::Value)>, DbContextError> {
        let cf = self.get_cf_handle::<CF>()?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::From(prefix, rocksdb::Direction::Forward));
        
        let mut results = Vec::new();
        for item in iter {
            let (key_bytes, value_bytes) = item?;
            
            // Stop when we move past our prefix
            if !key_bytes.starts_with(prefix) {
                break;
            }
            
            let key = CF::KeyCodec::decode(&key_bytes)?;
            let value = CF::ValueCodec::decode(&value_bytes)?;
            results.push((key, value));
        }
        
        Ok(results)
    }
    
    /// Perform a batch write operation
    pub fn batch_write<F>(&self, f: F) -> Result<(), DbContextError>
    where
        F: FnOnce(&mut TypedBatchWriter) -> Result<(), DbContextError>,
    {
        let mut batch = WriteBatch::default();
        let mut writer = TypedBatchWriter::new(&mut batch, &self.db);
        f(&mut writer)?;
        self.db.write(batch)?;
        Ok(())
    }
    
    /// Get column family handle for the specified TypedCf
    fn get_cf_handle<CF: TypedCf>(&self) -> Result<&rocksdb::ColumnFamily, DbContextError> {
        self.db
            .cf_handle(CF::NAME)
            .ok_or_else(|| DbContextError::ColumnFamilyNotFound(CF::NAME.to_string()))
    }
}

/// Batch writer for typed database operations
pub struct TypedBatchWriter<'a> {
    batch: &'a mut WriteBatch,
    db: &'a DB,
}

impl<'a> TypedBatchWriter<'a> {
    fn new(
        batch: &'a mut WriteBatch,
        db: &'a DB,
    ) -> Self {
        Self { batch, db }
    }
    
    /// Put a key-value pair in the batch for the specified column family
    pub fn put<CF: TypedCf>(&mut self, key: &CF::Key, value: &CF::Value) -> Result<(), DbContextError> {
        let key_bytes = CF::KeyCodec::encode(key)?;
        let value_bytes = CF::ValueCodec::encode(value)?;
        
        if let Some(cf) = self.db.cf_handle(CF::NAME) {
            self.batch.put_cf(&cf, key_bytes, value_bytes);
            Ok(())
        } else {
            Err(DbContextError::ColumnFamilyNotFound(CF::NAME.to_string()))
        }
    }
    
    /// Delete a key from the batch for the specified column family
    #[allow(dead_code)]
    pub fn delete<CF: TypedCf>(&mut self, key: &CF::Key) -> Result<(), DbContextError> {
        let key_bytes = CF::KeyCodec::encode(key)?;
        
        if let Some(cf) = self.db.cf_handle(CF::NAME) {
            self.batch.delete_cf(&cf, key_bytes);
            Ok(())
        } else {
            Err(DbContextError::ColumnFamilyNotFound(CF::NAME.to_string()))
        }
    }
}

// Tests are moved to test/ directory following project conventions