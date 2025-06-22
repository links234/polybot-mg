use crate::typed_store::codec::{DbCodec, RocksDbKey, RocksDbValue};

/// Column Family trait for type-safe RocksDB operations
/// Defines type-safe column family with codecs for keys and values
pub trait TypedCf {
    type Key: Clone + std::fmt::Debug;
    type Value: Clone + std::fmt::Debug;
    type KeyCodec: DbCodec<Self::Key>;
    type ValueCodec: DbCodec<Self::Value>;

    /// Column family name - must be unique across the database
    const NAME: &'static str;

    /// Single byte prefix for backward compatibility (0x00‑0xFF)
    const PREFIX: u8;
}

/// Legacy Table trait for backward compatibility
pub trait Table {
    type Key: RocksDbKey;
    type Value: RocksDbValue;

    /// Single byte prefix (0x00‑0xFF) allowing up to 256 tables.
    /// Each table must have a unique prefix to avoid key collisions.
    const PREFIX: u8;
}

/// Macro to define a TypedCf with default BincodeDbCodec
#[macro_export]
macro_rules! define_typed_cf {
    ($name:ident, $key:ty, $value:ty, $cf_name:literal, $prefix:expr) => {
        pub struct $name;

        impl $crate::typed_store::table::TypedCf for $name {
            type Key = $key;
            type Value = $value;
            type KeyCodec = $crate::typed_store::codec::BincodeDbCodec;
            type ValueCodec = $crate::typed_store::codec::BincodeDbCodec;
            const NAME: &'static str = $cf_name;
            const PREFIX: u8 = $prefix;
        }
    };
}

/// Macro to define a TypedCf with custom codecs
#[macro_export]
macro_rules! define_typed_cf_with_codecs {
    ($name:ident, $key:ty, $value:ty, $key_codec:ty, $value_codec:ty, $cf_name:literal, $prefix:expr) => {
        pub struct $name;

        impl $crate::typed_store::table::TypedCf for $name {
            type Key = $key;
            type Value = $value;
            type KeyCodec = $key_codec;
            type ValueCodec = $value_codec;
            const NAME: &'static str = $cf_name;
            const PREFIX: u8 = $prefix;
        }
    };
}

/// Legacy macro to help ensure unique prefixes at compile time
#[macro_export]
macro_rules! define_table {
    ($name:ident, $key:ty, $value:ty, $prefix:expr) => {
        pub struct $name;

        impl $crate::typed_store::table::Table for $name {
            type Key = $key;
            type Value = $value;
            const PREFIX: u8 = $prefix;
        }
    };
}
