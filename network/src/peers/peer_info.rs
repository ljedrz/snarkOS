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

use crate::NetworkError;

use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8},
        Arc,
    },
    time::Instant,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum PeerStatus {
    Connecting,
    Connected,
    Disconnected,
    NeverConnected,
}

#[derive(Debug, Default)]
pub struct PeerQuality {
    /// The timestamp of when the peer has been seen last.
    pub last_seen: RwLock<Option<DateTime<Utc>>>,
    /// An indicator of whether a `Pong` message is currently expected from this peer.
    pub expecting_pong: AtomicBool,
    /// The timestamp of the last `Ping` sent to the peer.
    pub last_ping_sent: Mutex<Option<Instant>>,
    /// The time it took to send a `Ping` to the peer and for it to respond with a `Pong`.
    pub rtt_ms: AtomicU64,
    /// The number of failures associated with the peer; grounds for dismissal.
    pub failures: AtomicU8,
}

/// A data structure containing information about a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// The IP address of this peer.
    address: SocketAddr,
    /// The current status of this peer.
    status: PeerStatus,
    /// The timestamp of the first seen instance of this peer.
    first_seen: Option<DateTime<Utc>>,
    /// The timestamp of the last seen instance of this peer.
    last_connected: Option<DateTime<Utc>>,
    /// The timestamp of the last disconnect from this peer.
    last_disconnected: Option<DateTime<Utc>>,
    /// The number of times we have connected to this peer.
    connected_count: u64,
    /// The number of times we have disconnected from this peer.
    disconnected_count: u64,
    /// The quality of the connection with the peer.
    #[serde(skip)]
    pub quality: Arc<PeerQuality>,
}

impl PeerInfo {
    ///
    /// Creates a new instance of `PeerInfo`.
    ///
    pub fn new(address: SocketAddr) -> Self {
        Self {
            address,
            status: PeerStatus::NeverConnected,
            first_seen: None,
            last_connected: None,
            last_disconnected: None,
            connected_count: 0,
            disconnected_count: 0,
            quality: Default::default(),
        }
    }

    ///
    /// Returns the IP address of this peer.
    ///
    #[inline]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    ///
    /// Returns the current status of this peer.
    ///
    #[inline]
    pub fn status(&self) -> PeerStatus {
        self.status
    }

    ///
    /// Returns the timestamp of the first seen instance of this peer.
    ///
    #[inline]
    pub fn first_seen(&self) -> Option<DateTime<Utc>> {
        self.first_seen
    }

    ///
    /// Returns the timestamp of the last seen instance of this peer.
    ///
    #[inline]
    pub fn last_seen(&self) -> Option<DateTime<Utc>> {
        *self.quality.last_seen.read()
    }

    ///
    /// Returns the timestamp of the last connection to this peer.
    ///
    #[inline]
    pub fn last_connected(&self) -> Option<DateTime<Utc>> {
        self.last_connected
    }

    ///
    /// Returns the timestamp of the last disconnect from this peer.
    ///
    #[inline]
    pub fn last_disconnected(&self) -> Option<DateTime<Utc>> {
        self.last_disconnected
    }

    ///
    /// Returns the number of times we have connected to this peer.
    ///
    #[inline]
    pub fn connected_count(&self) -> u64 {
        self.connected_count
    }

    ///
    /// Returns the number of times we have disconnected from this peer.
    ///
    #[inline]
    pub fn disconnected_count(&self) -> u64 {
        self.disconnected_count
    }

    ///
    /// Updates the peer to connecting and sets the handshake to the given nonce.
    ///
    /// If the peer is not transitioning from `PeerStatus::Disconnected` or `PeerStatus::NeverConnected`,
    /// this function returns a `NetworkError`.
    ///
    /// If there is a handshake already set, then this peer is already connected
    /// and this function returns a `NetworkError`.
    ///
    /// If the given handshake nonce has been used before, returns a `NetworkError`.
    ///
    pub fn set_connecting(&mut self) -> Result<(), NetworkError> {
        // Fetch the current status of the peer.
        match self.status() {
            PeerStatus::Disconnected | PeerStatus::NeverConnected => {
                // Set the status of this peer to connecting.
                self.status = PeerStatus::Connecting;

                if self.first_seen.is_none() {
                    self.first_seen = Some(Utc::now());
                }

                Ok(())
            }
            PeerStatus::Connecting | PeerStatus::Connected => {
                error!(
                    "Attempting to reconnect to a connecting or connected peer - {}",
                    self.address
                );
                Err(NetworkError::PeerAlreadyConnected)
            }
        }
    }

    ///
    /// Updates the peer to connected, if the given nonce matches the stored nonce.
    ///
    /// If the peer is not transitioning from `PeerStatus::Connecting`,
    /// this function returns a `NetworkError`.
    ///
    pub(crate) fn set_connected(&mut self) -> Result<(), NetworkError> {
        // Fetch the current status of the peer.
        match self.status() {
            PeerStatus::Connecting => {
                // Set the state of this peer to connected.
                self.status = PeerStatus::Connected;

                self.last_connected = Some(Utc::now());
                self.connected_count += 1;

                Ok(())
            }
            PeerStatus::Connected => {
                error!("Attempting to reconnect to a connected peer - {}", self.address);
                Err(NetworkError::PeerAlreadyConnected)
            }
            PeerStatus::Disconnected | PeerStatus::NeverConnected => Err(NetworkError::PeerIsDisconnected),
        }
    }

    ///
    /// Updates the peer to disconnected.
    ///
    /// If the peer is not transitioning from `PeerStatus::Connecting` or `PeerStatus::Connected`,
    /// this function returns a `NetworkError`.
    ///
    pub(crate) fn set_disconnected(&mut self) -> Result<(), NetworkError> {
        match self.status() {
            PeerStatus::Connected | PeerStatus::Connecting => {
                // Set the state of this peer to disconnected.
                self.status = PeerStatus::Disconnected;

                self.last_disconnected = Some(Utc::now());
                self.disconnected_count += 1;

                Ok(())
            }
            PeerStatus::Disconnected | PeerStatus::NeverConnected => {
                error!("Attempting to disconnect from a disconnected peer - {}", self.address);
                Err(NetworkError::PeerAlreadyDisconnected)
            }
        }
    }
}

#[cfg(tests)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let peer_info = PeerInfo::new(format!("127.0.0.1:4130").parse()?);
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::NeverConnected, peer_info.status());
        assert_eq!(&None, peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_connecting_from_never_connected() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::NeverConnected, peer_info.status());

        peer_info.set_connecting(&address, 0).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Connecting, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_connected_from_connecting() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);
        peer_info.set_connecting(&address, 0).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Connecting, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());

        peer_info.set_connected(&address).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Connected, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(1, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_disconnected_from_connecting() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);
        peer_info.set_connecting(&address, 0).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Connecting, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());

        peer_info.set_disconnected(&address).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Disconnected, peer_info.status());
        assert_eq!(&None, peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(1, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_disconnected_from_connected() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);
        peer_info.set_connecting(&address, 0).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Connecting, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());

        peer_info.set_connected(&address).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Connected, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(1, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());

        peer_info.set_disconnected(&address).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Disconnected, peer_info.status());
        assert_eq!(&None, peer_info.nonce());
        assert_eq!(1, peer_info.connected_count().len());
        assert_eq!(1, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_connected_from_never_connected() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);

        assert!(peer_info.set_connected(&address, 0).is_err());

        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::NeverConnected, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_disconnected_from_never_connected() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);

        assert!(peer_info.set_disconnected(&address).is_err());

        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::NeverConnected, peer_info.status());
        assert_eq!(&Some(0), peer_info.nonce());
        assert_eq!(0, peer_info.connected_count().len());
        assert_eq!(0, peer_info.disconnected_count().len());
    }

    #[test]
    fn test_set_connected_from_disconnected() {
        let address: SocketAddr = format!("127.0.0.1:4130").parse()?;

        let mut peer_info = PeerInfo::new(address);
        peer_info.set_connecting(&address, 0).unwrap();
        peer_info.set_connected(&address).unwrap();
        peer_info.set_disconnected(&address).unwrap();
        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Disconnected, peer_info.status());
        assert_eq!(&None, peer_info.nonce());
        assert_eq!(1, peer_info.connected_count().len());
        assert_eq!(1, peer_info.disconnected_count().len());

        assert!(peer_info.set_connected(&address).is_err());

        assert_eq!(address, peer_info.address());
        assert_eq!(PeerStatus::Disconnected, peer_info.status());
        assert_eq!(&None, peer_info.nonce());
        assert_eq!(1, peer_info.connected_count().len());
        assert_eq!(1, peer_info.disconnected_count().len());
    }
}
