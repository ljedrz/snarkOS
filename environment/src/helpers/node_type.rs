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

use std::fmt;

use anyhow::{self, bail};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum NodeTypeId {
    Client = 0,
    Miner,
    Beacon,
    Sync,
    Operator,
    Prover,
}

impl fmt::Display for NodeTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait NodeType {
    fn id() -> NodeTypeId;

    fn description() -> &'static str;
}

macro_rules! impl_node_type {
    ($t: ident, $desc: expr, $doc: expr) => {
        #[derive(Clone, Copy, Debug)]
        #[doc = $doc]
        pub struct $t;

        impl NodeType for $t {
            fn id() -> NodeTypeId {
                NodeTypeId::$t
            }

            fn description() -> &'static str {
                $desc
            }
        }

        impl PartialEq<NodeTypeId> for $t {
            fn eq(&self, other: &NodeTypeId) -> bool {
                <Self as NodeType>::id() == *other
            }
        }

        impl TryFrom<NodeTypeId> for $t {
            type Error = anyhow::Error;

            fn try_from(id: NodeTypeId) -> anyhow::Result<Self> {
                if id == $t::id() {
                    Ok(Self)
                } else {
                    bail!("Invalid node type id");
                }
            }
        }
    };
}

impl_node_type!(
    Client,
    "a client node",
    "A client node is a full node, capable of sending and receiving blocks."
);
impl_node_type!(
    Miner,
    "a mining node",
    "A mining node is a full node, capable of producing new blocks."
);
impl_node_type!(
    Beacon,
    "a beacon node",
    "A beacon node is a discovery node, capable of sharing peers of the network."
);
impl_node_type!(
    Sync,
    "a sync node",
    "A sync node is a discovery node, capable of syncing nodes for the network."
);
impl_node_type!(
    Operator,
    "an operator node",
    "An operating node is a full node, capable of coordinating provers in a pool."
);
impl_node_type!(
    Prover,
    "a prover node",
    "A proving node is a full node, capable of producing proofs for a pool."
);
