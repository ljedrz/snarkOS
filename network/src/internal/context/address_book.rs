// Copyright (C) 2019-2020 Aleo Systems Inc.
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

use chrono::{DateTime, Utc};
use std::{collections::HashMap, net::SocketAddr};

/// Stores the existence of a peer and the date they were last seen.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub(super) struct AddressBook {
    addresses: HashMap<SocketAddr, DateTime<Utc>>,
}

impl AddressBook {
    /// Construct a new `AddressBook`.
    pub fn new() -> Self {
        Self::default()
    }

    ///
    /// Insert or update a new date for an address.
    /// Returns `true` if the address is new and inserted.
    /// Returns `false` if the address already exists.
    ///
    /// If the address already exists in the address book,
    /// the datetime will be updated to reflect the latest datetime.
    ///
    pub fn insert_or_update(&mut self, address: SocketAddr, date: DateTime<Utc>) -> bool {
        match self.addresses.get(&address) {
            Some(stored_date) => {
                if stored_date < &date {
                    self.addresses.insert(address, date);
                }
                false
            }
            None => self.addresses.insert(address, date).is_none(),
        }
    }

    /// Returns true if address is stored in the mapping.
    pub fn contains(&self, address: &SocketAddr) -> bool {
        self.addresses.contains_key(address)
    }

    /// Remove an address mapping and return its last seen date.
    pub fn remove(&mut self, address: &SocketAddr) -> Option<DateTime<Utc>> {
        self.addresses.remove(address)
    }

    /// Returns the number of stored peers.
    pub fn length(&self) -> u16 {
        self.addresses.len() as u16
    }

    /// Returns copy of addresses
    pub fn get_addresses(&self) -> HashMap<SocketAddr, DateTime<Utc>> {
        self.addresses.clone()
    }
}
