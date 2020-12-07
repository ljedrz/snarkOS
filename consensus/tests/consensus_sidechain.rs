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

mod consensus_sidechain {
    use snarkos_consensus::MemoryPool;
    use snarkos_dpc::base_dpc::instantiated::Tx;
    use snarkos_objects::Block;
    use snarkos_testing::{consensus::*, dpc::load_verifying_parameters, storage::*};
    use snarkos_utilities::bytes::FromBytes;

    use std::sync::Arc;

    // Receive two new blocks out of order.
    // Like the test above, except block 2 is received first as an orphan with no parent.
    // The consensus mechanism should push the orphan into storage until block 1 is received.
    // After block 1 is received, block 2 should be fetched from storage and added to the chain.
    #[test]
    fn new_out_of_order() {
        let blockchain = Arc::new(FIXTURE_VK.ledger());
        let parameters = load_verifying_parameters();

        let mut memory_pool = MemoryPool::new();

        let consensus = TEST_CONSENSUS.clone();

        let old_block_height = blockchain.get_current_block_height();

        // Find second block

        let block_2 = Block::<Tx>::read(&BLOCK_2[..]).unwrap();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_2)
            .unwrap();

        // Find first block

        let block_1 = Block::<Tx>::read(&BLOCK_1[..]).unwrap();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1)
            .unwrap();

        // Check balances after both blocks

        let new_block_height = blockchain.get_current_block_height();
        assert_eq!(old_block_height + 2, new_block_height);

        kill_storage_sync(blockchain);
    }

    // Receive two blocks that reference the same parent.
    // Treat the first block received as the canonical chain but store and keep the rejected sidechain block in storage.
    #[test]
    fn reject() {
        let blockchain = Arc::new(FIXTURE_VK.ledger());
        let parameters = load_verifying_parameters();

        let mut memory_pool = MemoryPool::new();

        let consensus = TEST_CONSENSUS.clone();

        let block_1_canon = Block::<Tx>::read(&BLOCK_1[..]).unwrap();
        let block_1_side = Block::<Tx>::read(&ALTERNATIVE_BLOCK_1[..]).unwrap();

        let old_block_height = blockchain.get_current_block_height();

        // 1. Receive canonchain block 1.

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1_canon)
            .unwrap();

        // 2. Receive sidechain block 1.

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1_side)
            .unwrap();

        let new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height + 1, new_block_height);

        // 3. Ensure sidechain block 1 rejected.

        let accepted = blockchain.get_latest_block().unwrap();

        assert_ne!(accepted, block_1_side);

        kill_storage_sync(blockchain);
    }

    // Receive blocks from a sidechain that overtakes our current canonical chain.
    #[test]
    fn accept() {
        let blockchain = Arc::new(FIXTURE_VK.ledger());
        let parameters = load_verifying_parameters();

        let mut memory_pool = MemoryPool::new();

        let consensus = TEST_CONSENSUS.clone();

        let block_1_canon = Block::<Tx>::read(&ALTERNATIVE_BLOCK_1[..]).unwrap();
        let block_1_side = Block::<Tx>::read(&BLOCK_1[..]).unwrap();
        let block_2_side = Block::<Tx>::read(&BLOCK_2[..]).unwrap();

        // 1. Receive shorter chain of block_1_canon.

        let mut old_block_height = blockchain.get_current_block_height();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1_canon)
            .unwrap();

        let mut new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height + 1, new_block_height);

        // 2. Receive longer chain of blocks 1 and 2 from the sidechain (the longest chain wins).

        old_block_height = blockchain.get_current_block_height();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1_side)
            .unwrap();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_2_side)
            .unwrap();

        new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height + 1, new_block_height);

        kill_storage_sync(blockchain);
    }

    // Receive blocks from a sidechain (out of order) that overtakes our current canonical chain.
    #[test]
    fn fork_out_of_order() {
        let blockchain = Arc::new(FIXTURE_VK.ledger());
        let parameters = load_verifying_parameters();

        let mut memory_pool = MemoryPool::new();

        let consensus = TEST_CONSENSUS.clone();

        let block_1_canon = Block::<Tx>::read(&BLOCK_1[..]).unwrap();
        let block_2_canon = Block::<Tx>::read(&BLOCK_2[..]).unwrap();
        let block_1_side = Block::<Tx>::read(&ALTERNATIVE_BLOCK_1[..]).unwrap();
        let block_2_side = Block::<Tx>::read(&ALTERNATIVE_BLOCK_2[..]).unwrap();

        // 1. Receive irrelevant block.

        let mut old_block_height = blockchain.get_current_block_height();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_2_canon)
            .unwrap();

        let mut new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height, new_block_height);

        // 2. Receive valid sidechain block

        old_block_height = blockchain.get_current_block_height();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1_side)
            .unwrap();

        new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height + 1, new_block_height);

        // 3. Receive valid canon block 1 and accept the previous irrelevant block 2

        old_block_height = blockchain.get_current_block_height();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_1_canon)
            .unwrap();

        new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height + 1, new_block_height);

        // 4. Receive valid canon block 1 and accept the previous irrelevant block 2

        old_block_height = blockchain.get_current_block_height();

        consensus
            .receive_block(&parameters, &blockchain, &mut memory_pool, &block_2_side)
            .unwrap();

        new_block_height = blockchain.get_current_block_height();

        assert_eq!(old_block_height, new_block_height);

        kill_storage_sync(blockchain);
    }
}
