// Copyright (C) 2019-2022 Aleo Systems Inc.
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

mod common;
use common::*;

use std::{net::SocketAddr, time::Duration};

use deadline::deadline;
use pea2pea::{protocols::*, Pea2Pea};
use snarkos_node::Node;
use snarkvm::prelude::{PrivateKey, TestRng};

#[tokio::test]
async fn handshake_initiator_side() {
    let mut rng = TestRng::default();

    let synth_node = SyntheticNode::new().await;
    synth_node.enable_handshake().await;
    let synth_node_addr = synth_node.node().listening_addr().unwrap();

    let full_node_pk = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
    let full_node_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let _full_node = Node::new_client(full_node_addr, full_node_pk, &[synth_node_addr]).await;

    deadline!(Duration::from_secs(1), move || synth_node.node().num_connected() == 1);

    // TODO: check snarkOS node side
}

#[tokio::test]
async fn handshake_responder_side() {
    let mut rng = TestRng::default();

    let synth_node = SyntheticNode::new().await;
    synth_node.enable_handshake().await;

    let full_node_pk = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
    let full_node_addr: SocketAddr = "127.0.0.1:4130".parse().unwrap(); // TODO: unfix the port
    let _full_node = Node::new_client(full_node_addr, full_node_pk, &[]).await;

    synth_node.node().connect(full_node_addr).await.unwrap();

    // TODO: check snarkOS node side
}
