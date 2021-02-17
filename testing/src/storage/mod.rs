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

use crate::consensus::TestTx;
pub use snarkos_storage::{Ledger, RocksDb};
use snarkvm_dpc::base_dpc::instantiated::CommitmentMerkleParameters;
use snarkvm_models::{
    algorithms::merkle_tree::LoadableMerkleParameters,
    objects::{LedgerScheme, Storage, Transaction},
};
use snarkvm_objects::Block;

use rand::{thread_rng, Rng};
use std::path::PathBuf;

pub type Store = Ledger<TestTx, CommitmentMerkleParameters, RocksDb>; // TODO(ljedrz): change to the in-mem storage

pub fn random_storage_path() -> String {
    let random_path: usize = thread_rng().gen();
    format!("./test_db-{}", random_path)
}

// Initialize a test blockchain given genesis attributes
pub fn initialize_test_blockchain<T: Transaction, P: LoadableMerkleParameters, S: Storage>(
    parameters: P,
    genesis_block: Block<T>,
) -> Ledger<T, P, S> {
    let mut path = std::env::temp_dir();
    path.push(random_storage_path());

    Ledger::<T, P, S>::new(Some(&path), parameters, genesis_block).unwrap()
}

// Open a test blockchain from stored genesis attributes
pub fn open_test_blockchain<T: Transaction, P: LoadableMerkleParameters, S: Storage>() -> (Ledger<T, P, S>, PathBuf) {
    let mut path = std::env::temp_dir();
    path.push(random_storage_path());

    let storage = Ledger::<T, P, S>::open_at_path(path.clone()).unwrap();

    (storage, path)
}
