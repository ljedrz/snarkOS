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

use crate::{
    external::message_types::{Peers as PeersStruct, *},
    outbound::Request,
    peers::{PeerBook, PeerInfo},
    Environment,
    Inbound,
    NetworkError,
    Outbound,
};

// TODO (howardwu): Move these imports to SyncManager.
use snarkos_consensus::{memory_pool::Entry, ConsensusParameters, MerkleTreeLedger};
use snarkos_dpc::base_dpc::{
    instantiated::{Components, Tx},
    parameters::PublicParameters,
};
use snarkos_objects::Block as BlockStruct;
use snarkos_utilities::{
    bytes::{FromBytes, ToBytes},
    to_bytes,
};

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::{sync::RwLock, task};

/// A stateful component for managing the blocks for the ledger on this node server.
#[derive(Clone)]
pub struct Blocks {
    /// The parameters and settings of this node server.
    environment: Environment,
    /// The outbound handler of this node server.
    outbound: Arc<Outbound>,
}

impl Blocks {
    ///
    /// Creates a new instance of `Blocks`.
    ///
    #[inline]
    pub fn new(environment: &mut Environment, outbound: Arc<Outbound>) -> Result<Self, NetworkError> {
        trace!("Instantiating block service");
        Ok(Self {
            environment: environment.clone(),
            outbound,
        })
    }

    ///
    /// Broadcasts updates with connected peers and maintains a permitted number of connected peers.
    ///
    #[inline]
    pub async fn update(&self) -> Result<(), NetworkError> {
        debug!("Updating block service");

        // Check that this node is not a bootnode.
        if !self.environment.is_bootnode() {}

        debug!("Updated block service");
        Ok(())
    }

    ///
    /// Returns the local address of this node.
    ///
    #[inline]
    pub fn local_address(&self) -> SocketAddr {
        // TODO (howardwu): Check that env addr and peer book addr match.
        // // Acquire the peer book reader.
        // let peer_book = self.peer_book.read().await;
        // // Fetch the local address of this node.
        // peer_book.local_address()

        *self.environment.local_address()
    }

    /// TODO (howardwu): Move this to the SyncManager.
    /// Broadcast block to connected peers
    pub(crate) async fn propagate_block(
        &self,
        block_bytes: Vec<u8>,
        block_miner: SocketAddr,
        connected_peers: &HashMap<SocketAddr, PeerInfo>,
    ) -> Result<(), NetworkError> {
        debug!("Propagating a block to peers");

        let local_address = self.local_address();
        for (remote_address, _) in connected_peers {
            if *remote_address != block_miner && *remote_address != local_address {
                // Broadcast a `Block` message to the connected peer.
                self.outbound
                    .broadcast(&Request::Block(*remote_address, Block::new(block_bytes.clone())))
                    .await;

                // if let Some(channel) = peers.get_channel(&remote_address) {
                //     match channel.write(&).await {
                //         Ok(_) => num_peers += 1,
                //         Err(error) => warn!(
                //             "Failed to propagate block to peer {}. (error message: {})",
                //             channel.address, error
                //         ),
                //     }
                // }
            }
        }

        Ok(())
    }

    /// TODO (howardwu): Move this to the SyncManager.
    /// Broadcast transaction to connected peers
    pub(crate) async fn propagate_transaction(
        &self,
        transaction_bytes: Vec<u8>,
        transaction_sender: SocketAddr,
        connected_peers: &HashMap<SocketAddr, PeerInfo>,
    ) -> Result<(), NetworkError> {
        debug!("Propagating a transaction to peers");

        let local_address = self.local_address();

        for (remote_address, _) in connected_peers {
            if *remote_address != transaction_sender && *remote_address != local_address {
                // Broadcast a `Block` message to the connected peer.
                self.outbound
                    .broadcast(&Request::Transaction(
                        *remote_address,
                        Transaction::new(transaction_bytes.clone()),
                    ))
                    .await;

                // if let Some(channel) = connections.get_channel(&socket) {
                //     match channel.write(&Transaction::new(transaction_bytes.clone())).await {
                //         Ok(_) => num_peers += 1,
                //         Err(error) => warn!(
                //             "Failed to propagate transaction to peer {}. (error message: {})",
                //             channel.address, error
                //         ),
                //     }
                // }
            }
        }

        Ok(())
    }

    /// TODO (howardwu): Move this to the SyncManager.
    /// Verify a transaction, add it to the memory pool, propagate it to peers.
    pub(crate) async fn received_transaction(
        &self,
        source: SocketAddr,
        transaction: Transaction,
        connected_peers: HashMap<SocketAddr, PeerInfo>,
    ) -> Result<(), NetworkError> {
        if let Ok(tx) = Tx::read(&transaction.bytes[..]) {
            let mut memory_pool = self.environment.memory_pool().lock().await;
            let parameters = self.environment.dpc_parameters();
            let storage = self.environment.storage();
            let consensus = self.environment.consensus_parameters();

            if !consensus.verify_transaction(parameters, &tx, &*storage.read().await)? {
                error!("Received a transaction that was invalid");
                return Ok(());
            }

            if tx.value_balance.is_negative() {
                error!("Received a transaction that was a coinbase transaction");
                return Ok(());
            }

            let entry = Entry::<Tx> {
                size_in_bytes: transaction.bytes.len(),
                transaction: tx,
            };

            if let Ok(inserted) = memory_pool.insert(&*storage.read().await, entry) {
                if inserted.is_some() {
                    info!("Transaction added to memory pool.");
                    self.propagate_transaction(transaction.bytes, source, &connected_peers)
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// A peer has sent us a new block to process.
    #[inline]
    pub(crate) async fn received_block(
        &self,
        remote_address: SocketAddr,
        block: Block,
        connected_peers: Option<HashMap<SocketAddr, PeerInfo>>,
    ) -> Result<(), NetworkError> {
        let block_struct = BlockStruct::deserialize(&block.data)?;
        info!(
            "Received block from epoch {} with hash {:?}",
            block_struct.header.time,
            hex::encode(block_struct.header.get_hash().0)
        );

        // Verify the block and insert it into the storage.
        if !self
            .environment
            .storage_read()
            .await
            .block_hash_exists(&block_struct.header.get_hash())
        {
            let is_new_block = self
                .environment
                .consensus_parameters()
                .receive_block(
                    self.environment.dpc_parameters(),
                    &*self.environment.storage_read().await,
                    &mut *self.environment.memory_pool().lock().await,
                    &block_struct,
                )
                .is_ok();

            // This is a new block, send it to our peers.
            if let Some(connected_peers) = connected_peers {
                if is_new_block {
                    self.propagate_block(block.data, remote_address, &connected_peers)
                        .await?;
                }
            } else {
                // if let Ok(mut sync_manager) = self.environment.sync_manager().await.try_lock() {
                //     // TODO (howardwu): Implement this.
                //     {
                //         // sync_manager.clear_pending().await;
                //         //
                //         // if sync_manager.sync_state != SyncState::Idle {
                //         //     // We are currently syncing with a node, ask for the next block.
                //         //     if let Some(channel) = environment
                //         //         .peers_read()
                //         //         .await
                //         //         .get_channel(&sync_manager.sync_node_address)
                //         //     {
                //         //         sync_manager.increment(channel.clone()).await?;
                //         //     }
                //         // }
                //     }
                // }
            }
        }

        Ok(())
    }

    /// A peer has requested a block.
    pub(crate) async fn received_get_block(
        &self,
        remote_address: SocketAddr,
        message: GetBlock,
    ) -> Result<(), NetworkError> {
        if let Ok(block) = self.environment.storage_read().await.get_block(&message.block_hash) {
            // Broadcast a `SyncBlock` message to the connected peer.
            self.outbound
                .broadcast(&Request::SyncBlock(remote_address, SyncBlock::new(block.serialize()?)))
                .await;
        }
        Ok(())
    }

    /// A peer has requested our memory pool transactions.
    pub(crate) async fn received_get_memory_pool(
        &self,
        remote_address: SocketAddr,
        message: GetBlock,
    ) -> Result<(), NetworkError> {
        // TODO (howardwu): This should have been written with Rayon - it is easily parallelizable.
        let mut transactions = vec![];
        let memory_pool = self.environment.memory_pool().lock().await;
        for (_tx_id, entry) in &memory_pool.transactions {
            if let Ok(transaction_bytes) = to_bytes![entry.transaction] {
                transactions.push(transaction_bytes);
            }
        }

        if !transactions.is_empty() {
            // Broadcast a `MemoryPool` message to the connected peer.
            self.outbound
                .broadcast(&Request::MemoryPool(remote_address, MemoryPool::new(transactions)))
                .await;
        }

        Ok(())
    }

    /// A peer has sent us their memory pool transactions.
    pub(crate) async fn receive_memory_pool(&self, message: MemoryPool) -> Result<(), NetworkError> {
        let mut memory_pool = self.environment.memory_pool().lock().await;

        for transaction_bytes in message.transactions {
            let transaction: Tx = Tx::read(&transaction_bytes[..])?;
            let entry = Entry::<Tx> {
                size_in_bytes: transaction_bytes.len(),
                transaction,
            };

            if let Ok(inserted) = memory_pool.insert(&*self.environment.storage_read().await, entry) {
                if let Some(txid) = inserted {
                    debug!("Transaction added to memory pool with txid: {:?}", hex::encode(txid));
                }
            }
        }

        Ok(())
    }
}
