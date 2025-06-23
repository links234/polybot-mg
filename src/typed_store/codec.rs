use serde::{Deserialize, Serialize};
use std::fmt;

/// Database codec trait for encoding/decoding types to/from bytes
pub trait DbCodec<T> {
    fn encode(obj: &T) -> Result<Vec<u8>, CodecError>;
    fn decode(data: &[u8]) -> Result<T, CodecError>;
}

/// Default codec using bincode for serialization
#[derive(Debug, Clone)]
pub struct BincodeDbCodec;

impl<T> DbCodec<T> for BincodeDbCodec
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    fn encode(obj: &T) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(obj).map_err(|e| CodecError::SerializationError(e.to_string()))
    }

    fn decode(data: &[u8]) -> Result<T, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

/// JSON codec for human-readable serialization (useful for debugging)
#[derive(Debug, Clone)]
pub struct JsonDbCodec;

impl<T> DbCodec<T> for JsonDbCodec
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    fn encode(obj: &T) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(obj).map_err(|e| CodecError::SerializationError(e.to_string()))
    }

    fn decode(data: &[u8]) -> Result<T, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

/// Legacy traits for backward compatibility
pub trait RocksDbKey: Sized + Clone + fmt::Debug {
    fn encode_key(&self) -> Vec<u8>;
    fn decode_key(data: &[u8]) -> Result<Self, CodecError>;
}

pub trait RocksDbValue: Sized + Clone + fmt::Debug {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError>;
    fn decode_value(data: &[u8]) -> Result<Self, CodecError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Failed to serialize data: {0}")]
    SerializationError(String),
    #[error("Failed to deserialize data: {0}")]
    DeserializationError(String),
    #[error("Invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

// Blanket implementations for common key types

impl RocksDbKey for String {
    fn encode_key(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn decode_key(data: &[u8]) -> Result<Self, CodecError> {
        String::from_utf8(data.to_vec()).map_err(CodecError::from)
    }
}

impl RocksDbKey for &str {
    fn encode_key(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn decode_key(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::DeserializationError(
            "&str cannot be decoded without allocation; use String".to_string(),
        ))
    }
}

impl RocksDbKey for u64 {
    fn encode_key(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }

    fn decode_key(data: &[u8]) -> Result<Self, CodecError> {
        if data.len() != 8 {
            return Err(CodecError::DeserializationError(format!(
                "Expected 8 bytes for u64, got {}",
                data.len()
            )));
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(data);
        Ok(u64::from_be_bytes(bytes))
    }
}
