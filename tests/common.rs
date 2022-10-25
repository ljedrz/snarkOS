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

#![allow(dead_code)]

use std::{collections::HashMap, io, net::SocketAddr, sync::Arc};

use futures_util::{sink::SinkExt, TryFutureExt, TryStreamExt};
use parking_lot::Mutex;
use pea2pea::{protocols::*, Config, Connection, ConnectionSide, Pea2Pea};
use snarkos_node_executor::{NodeType, Status};
use snarkos_node_messages::{
    ChallengeRequest,
    ChallengeResponse,
    Data,
    Message as SnarkosMessage,
    MessageCodec as SnarkosCodec,
    Ping,
    Pong,
};
use snarkvm::prelude::{Block, FromBytes, Network, Testnet3};
use tokio_util::codec::Framed;
use tracing::*;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

pub type CurrentNetwork = Testnet3;

#[derive(Clone)]
pub struct SyntheticNode {
    node: pea2pea::Node,
    peers: Arc<Mutex<HashMap<SocketAddr, Peer>>>,
}

impl Pea2Pea for SyntheticNode {
    fn node(&self) -> &pea2pea::Node {
        &self.node
    }
}

impl SyntheticNode {
    pub async fn new() -> Self {
        let config = Config { listener_ip: Some("127.0.0.1".parse().unwrap()), ..Default::default() };

        Self { node: pea2pea::Node::new(config).await.unwrap(), peers: Default::default() }
    }

    fn connected_addr_to_listening_addr(&self, addr: SocketAddr) -> Option<SocketAddr> {
        self.peers
            .lock()
            .iter()
            .find(|(_, peer)| peer.connected_addr == addr)
            .map(|(listening_addr, _)| *listening_addr)
    }
}

#[derive(Clone)]
struct Peer {
    connected_addr: SocketAddr,
    listening_addr: SocketAddr,
}

#[async_trait::async_trait]
impl Handshake for SyntheticNode {
    async fn perform_handshake(&self, mut conn: Connection) -> io::Result<Connection> {
        let node_conn_side = !conn.side();
        let peer_addr = conn.addr();

        debug!(parent: self.node().span(), "Performing a handshake with {peer_addr}");

        let mut handshake_stream = Framed::new(self.borrow_stream(&mut conn), SnarkosCodec::default());

        let genesis_header =
            Block::<CurrentNetwork>::from_bytes_le(CurrentNetwork::genesis_bytes()).unwrap().header().clone();
        let our_challenge = SnarkosMessage::<CurrentNetwork>::ChallengeRequest(ChallengeRequest {
            version: SnarkosMessage::<CurrentNetwork>::VERSION,
            fork_depth: 4096, // TODO: link to the const once it's moved
            node_type: NodeType::Client,
            status: Status::Ready,
            listener_port: self.node().listening_addr().unwrap().port(),
        });
        let our_challenge_response =
            SnarkosMessage::ChallengeResponse(ChallengeResponse { header: Data::Object(genesis_header) });

        let peer_listening_port = match node_conn_side {
            ConnectionSide::Initiator => {
                debug!(parent: self.node().span(), "sending a challenge request to {peer_addr}");
                handshake_stream.send(our_challenge).await?;

                let peers_challenge = handshake_stream.try_next().await?.ok_or(io::ErrorKind::InvalidData)?;

                let peer_listening_port =
                    if let SnarkosMessage::ChallengeRequest(ChallengeRequest { listener_port, .. }) = peers_challenge {
                        debug!(parent: self.node().span(), "received a challenge request from {peer_addr}");
                        listener_port
                    } else {
                        return Err(io::ErrorKind::InvalidData.into());
                    };

                debug!(parent: self.node().span(), "sending a challenge response to {peer_addr}");
                handshake_stream.send(our_challenge_response).await?;

                let peers_challenge_response = handshake_stream.try_next().await?.ok_or(io::ErrorKind::InvalidData)?;

                let _block_header = if let SnarkosMessage::ChallengeResponse(response) = peers_challenge_response {
                    debug!(parent: self.node().span(), "received a challenge response from {peer_addr}");
                    response.header.deserialize().map_err(|_| io::ErrorKind::InvalidData).await?
                } else {
                    return Err(io::ErrorKind::InvalidData.into());
                };

                peer_listening_port
            }
            ConnectionSide::Responder => {
                let peers_challenge = handshake_stream.try_next().await?.ok_or(io::ErrorKind::InvalidData)?;

                let peer_listening_port =
                    if let SnarkosMessage::ChallengeRequest(ChallengeRequest { listener_port, .. }) = peers_challenge {
                        debug!(parent: self.node().span(), "received a challenge request from {peer_addr}");
                        listener_port
                    } else {
                        return Err(io::ErrorKind::InvalidData.into());
                    };

                debug!(parent: self.node().span(), "sending a challenge request to {peer_addr}");
                handshake_stream.send(our_challenge).await?;

                debug!(parent: self.node().span(), "sending a challenge response to {peer_addr}");
                handshake_stream.send(our_challenge_response).await?;

                let peers_challenge_response = handshake_stream.try_next().await?.ok_or(io::ErrorKind::InvalidData)?;

                let _block_header = if let SnarkosMessage::ChallengeResponse(response) = peers_challenge_response {
                    debug!(parent: self.node().span(), "received a challenge response from {peer_addr}");
                    response.header.deserialize().map_err(|_| io::ErrorKind::InvalidData).await?
                } else {
                    return Err(io::ErrorKind::InvalidData.into());
                };

                peer_listening_port
            }
        };

        let peer_listening_addr = SocketAddr::from((peer_addr.ip(), peer_listening_port));

        {
            let mut locked_peers = self.peers.lock();

            if locked_peers.contains_key(&peer_listening_addr) {
                return Err(io::ErrorKind::AlreadyExists.into());
            }

            let peer = Peer { connected_addr: peer_addr, listening_addr: peer_listening_addr };

            locked_peers.insert(peer_listening_addr, peer);
        }

        let ping = SnarkosMessage::Ping(Ping {
            version: SnarkosMessage::<CurrentNetwork>::VERSION,
            fork_depth: 4096, // TODO: link to the const once it's moved
            node_type: NodeType::Client,
            status: Status::Ready,
        });

        debug!(parent: self.node().span(), "sending a ping to {peer_addr}");
        handshake_stream.send(ping).await?;

        Ok(conn)
    }
}

#[async_trait::async_trait]
impl Reading for SyntheticNode {
    type Codec = SnarkosCodec<CurrentNetwork>;
    type Message = SnarkosMessage<CurrentNetwork>;

    fn codec(&self, _addr: SocketAddr, _side: ConnectionSide) -> Self::Codec {
        Default::default()
    }

    async fn process_message(&self, source: SocketAddr, message: Self::Message) -> io::Result<()> {
        println!("got a message from {source}: {:?}", message);

        match message {
            SnarkosMessage::Ping(..) => {
                let pong = SnarkosMessage::Pong(Pong { is_fork: None });
                let _ = self.unicast(source, pong);
            }
            SnarkosMessage::Disconnect(reason) => {
                debug!("{source} gave the following reason for disconnecting: {:?}", reason);
            }
            _ => {}
        }

        Ok(())
    }
}

impl Writing for SyntheticNode {
    type Codec = SnarkosCodec<CurrentNetwork>;
    type Message = SnarkosMessage<CurrentNetwork>;

    fn codec(&self, _addr: SocketAddr, _side: ConnectionSide) -> Self::Codec {
        Default::default()
    }
}

#[async_trait::async_trait]
impl Disconnect for SyntheticNode {
    async fn handle_disconnect(&self, addr: SocketAddr) {
        debug!("Disconnecting from {addr}");

        if let Some(listening_addr) = self.connected_addr_to_listening_addr(addr) {
            self.peers.lock().remove(&listening_addr);
        }
    }
}

pub fn start_logger(default_level: LevelFilter) {
    let filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter.add_directive("tokio_util=off".parse().unwrap()),
        _ => EnvFilter::default().add_directive(default_level.into()).add_directive("tokio_util=off".parse().unwrap()),
    };

    tracing_subscriber::fmt().with_env_filter(filter).without_time().with_target(false).init();
}
