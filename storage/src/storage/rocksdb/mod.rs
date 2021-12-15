// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkOS library.

// The snarkOS library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkOS library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkOS library. If not, see <https://www.gnu.org/licenses/>.

mod iterator;
use iterator::*;

mod keys;
use keys::*;

mod map;
pub use map::*;

mod values;
use values::*;

#[cfg(test)]
mod tests;

use crate::storage::{Map, Storage};

use anyhow::Result;
use serde::{
    de::{self, DeserializeOwned},
    ser::SerializeSeq,
    Deserializer,
    Serialize,
    Serializer,
};
use std::{borrow::Borrow, fmt, marker::PhantomData, path::Path, sync::Arc};

///
/// An instance of a RocksDB database.
///
#[derive(Clone)]
pub struct RocksDB {
    rocksdb: Arc<rocksdb::DB>,
    network_id: u16,
    is_read_only: bool,
}

impl Storage for RocksDB {
    ///
    /// Opens storage at the given `path` and `context`.
    ///
    fn open<P: AsRef<Path>>(path: P, network_id: u16, is_read_only: bool) -> Result<Self> {
        // Customize database options.
        let mut options = rocksdb::Options::default();

        // FIXME: shorten the prefixes and make them the same length
        let prefix_extractor = rocksdb::SliceTransform::create_fixed_prefix(4);
        options.set_prefix_extractor(prefix_extractor);
        options.set_memtable_prefix_bloom_ratio(0.05);
        options.set_memtable_whole_key_filtering(true);
        options.set_bloom_locality(1);

        let primary = path.as_ref().to_path_buf();
        let rocksdb = match is_read_only {
            true => {
                // Construct the directory paths.
                let reader = path.as_ref().join("reader");
                // Open a secondary reader for the primary rocksdb.
                let rocksdb = rocksdb::DB::open_as_secondary(&options, &primary, &reader)?;
                Arc::new(rocksdb)
            }
            false => {
                options.increase_parallelism(2);
                options.create_if_missing(true);
                Arc::new(rocksdb::DB::open(&options, &primary)?)
            }
        };

        Ok(RocksDB {
            rocksdb,
            network_id,
            is_read_only,
        })
    }

    ///
    /// Opens a map with the given `context` from storage.
    ///
    fn open_map<K: Serialize + DeserializeOwned, V: Serialize + DeserializeOwned>(&self, map_id: MapId) -> Result<DataMap<K, V>> {
        let mut prefix = [0u8; 4];
        prefix[..2].copy_from_slice(&self.network_id.to_le_bytes());
        prefix[2..].copy_from_slice(&(map_id as u16).to_le_bytes());

        Ok(DataMap {
            rocksdb: self.rocksdb.clone(),
            prefix,
            is_read_only: self.is_read_only,
            _phantom: PhantomData,
        })
    }

    ///
    /// Imports the given serialized bytes to reconstruct storage.
    ///
    fn import<'de, D: Deserializer<'de>>(&self, deserializer: D) -> Result<(), D::Error> {
        struct RocksDBVisitor {
            rocksdb: RocksDB,
        }

        impl<'de> de::Visitor<'de> for RocksDBVisitor {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a rocksdb seq")
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
                while let Some((key, value)) = map.next_element::<(Vec<_>, Vec<_>)>()? {
                    self.rocksdb.rocksdb.put(&key, &value).map_err(serde::de::Error::custom)?;
                }

                Ok(())
            }
        }

        deserializer.deserialize_seq(RocksDBVisitor { rocksdb: self.clone() })?;

        Ok(())
    }

    ///
    /// Exports the current state of storage into serialized bytes.
    ///
    fn export(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(&self)?)
    }
}

impl Serialize for RocksDB {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut iterator = self.rocksdb.raw_iterator();
        iterator.seek_to_first();

        let mut map = serializer.serialize_seq(None)?;
        while iterator.valid() {
            if let (Some(key), Some(value)) = (iterator.key(), iterator.value()) {
                map.serialize_element(&(key, value))?;
            }
            iterator.next();
        }
        map.end()
    }
}
