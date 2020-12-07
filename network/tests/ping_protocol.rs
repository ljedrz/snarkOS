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

mod ping_protocol {
    use snarkos_network::external::{
        message_types::{Ping, Pong},
        Message,
        PingPongManager,
        PingPongWorker,
        PingState,
    };
    use snarkos_testing::network::{accept_channel, connect_channel, random_socket_address};

    use serial_test::serial;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    #[tokio::test]
    #[serial]
    async fn test_pings() {
        let local_address = random_socket_address();
        let remote_address = random_socket_address();

        // 1. Bind to server address

        let mut local_listener = TcpListener::bind(local_address).await.unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut remote_listener = TcpListener::bind(remote_address).await.unwrap();

            // 2. Peer connects to server address

            let channel = Arc::new(connect_channel(&mut remote_listener, local_address).await);

            // 4. Peer sends ping request

            let mut pings = PingPongManager::new();
            pings.send_ping(&channel).await.unwrap();
            assert_eq!(PingState::Waiting, pings.get_state(local_address).unwrap());

            // 7. Peer receives pong response

            let (_name, bytes) = channel.read().await.unwrap();
            let pong = Pong::deserialize(bytes).unwrap();

            pings.accept_pong(channel.address, pong).await.unwrap();

            assert_eq!(PingState::Accepted, pings.get_state(local_address).unwrap());
            tx.send(()).unwrap();
        });

        // 3. Server accepts peer connection

        let channel = Arc::new(accept_channel(&mut local_listener, remote_address).await);

        // 5. Server receives ping request

        let (_name, bytes) = channel.read().await.unwrap();
        let ping = Ping::deserialize(bytes).unwrap();

        // 6. Server sends pong response

        PingPongManager::send_pong(ping, channel).await.unwrap();
        rx.await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_ping_protocol() {
        let local_address = random_socket_address();
        let remote_address = random_socket_address();

        // 1. Bind listener to Server address

        let mut local_listener = TcpListener::bind(local_address).await.unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut remote_listener = TcpListener::bind(remote_address).await.unwrap();

            // 2. Peer connects to server address

            let channel = Arc::new(connect_channel(&mut remote_listener, local_address).await);

            // 4. Peer send ping request

            let mut peer_ping = PingPongWorker::send(&channel).await.unwrap();

            // 5. Peer accepts server pong response

            let (name, bytes) = channel.read().await.unwrap();

            assert_eq!(Pong::name(), name);

            peer_ping.accept(Pong::deserialize(bytes).unwrap()).await.unwrap();

            tx.send(()).unwrap();
        });

        // 3. Server accepts Peer connection

        let channel = Arc::new(accept_channel(&mut local_listener, remote_address).await);

        // 4. Server receives peer ping request. Sends pong response

        let (name, bytes) = channel.read().await.unwrap();

        assert_eq!(Ping::name(), name);

        PingPongWorker::receive(Ping::deserialize(bytes).unwrap(), channel)
            .await
            .unwrap();

        rx.await.unwrap();
    }
}
