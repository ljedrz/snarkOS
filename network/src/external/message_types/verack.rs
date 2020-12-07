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

use crate::external::message::{Message, MessageName};
use snarkos_errors::network::message::MessageError;

use std::net::SocketAddr;

#[cfg_attr(nightly, doc(include = "../../../documentation/network_messages/verack.md"))]
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Verack {
    /// Random nonce sequence number
    pub nonce: u64,

    /// Network address of sending node
    pub sender: SocketAddr,

    /// Network address of sending node
    pub receiver: SocketAddr,
}

impl Verack {
    pub fn new(nonce: u64, sender: SocketAddr, receiver: SocketAddr) -> Self {
        Self {
            nonce,
            sender,
            receiver,
        }
    }
}

impl Message for Verack {
    fn name() -> MessageName {
        MessageName::from("verack")
    }

    fn deserialize(vec: Vec<u8>) -> Result<Self, MessageError> {
        if vec.len() != 28 {
            return Err(MessageError::InvalidLength(vec.len(), 28));
        }

        Ok(Self {
            nonce: bincode::deserialize(&vec[0..8])?,
            receiver: bincode::deserialize(&vec[8..18])?,
            sender: bincode::deserialize(&vec[18..28])?,
        })
    }

    fn serialize(&self) -> Result<Vec<u8>, MessageError> {
        let mut writer = vec![];
        writer.extend_from_slice(&bincode::serialize(&self.nonce)?);
        writer.extend_from_slice(&bincode::serialize(&self.receiver)?);
        writer.extend_from_slice(&bincode::serialize(&self.sender)?);
        Ok(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::message_types::Version;
    use snarkos_testing::network::random_socket_address;

    #[test]
    fn test_verack() {
        let version = Version::new_with_rng(1u64, 1u32, random_socket_address(), random_socket_address());

        let message = Verack::new(version.nonce, version.receiver, version.sender);

        let serialized = message.serialize().unwrap();
        let deserialized = Verack::deserialize(serialized).unwrap();

        assert_eq!(message, deserialized);
    }
}
