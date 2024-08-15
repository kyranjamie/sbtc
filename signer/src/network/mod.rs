//! # Signer network interface
//!
//! This module provides the MessageTransfer trait that the signer implementation
//! will rely on for inter-signer communication, along with an in-memory
//! implementation of this trait for testing purposes.

pub mod grpc_relay;
pub mod in_memory;
pub mod libp2p_impl;

use std::future::Future;

use crate::ecdsa;
use crate::message;

/// The supported message type of the signer network
pub type Msg = ecdsa::Signed<message::SignerMessage>;

/// Represents the interaction point between signers and the signer network,
/// allowing signers to exchange messages with each other.
pub trait MessageTransfer {
    /// Errors occuring during either [`MessageTransfer::broadcast`] or [`MessageTransfer::receive`]
    type Error: std::error::Error;
    /// Send `msg` to all other signers
    fn broadcast(&mut self, msg: Msg) -> impl Future<Output = Result<(), Self::Error>> + Send;
    /// Receive a message from the network
    fn receive(&mut self) -> impl Future<Output = Result<Msg, Self::Error>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing;

    #[tokio::test]
    async fn two_clients_should_be_able_to_exchange_messages_given_an_in_memory_network() {
        let network = in_memory::Network::new();

        let client_1 = network.connect();
        let client_2 = network.connect();

        testing::network::assert_clients_can_exchange_messages(client_1, client_2).await;
    }

    #[tokio::test]
    async fn two_clients_should_be_able_to_exchange_messages_given_a_libp2p_network() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let keypair_1 = libp2p::identity::Keypair::generate_ed25519();
        let keypair_2 = libp2p::identity::Keypair::generate_ed25519();

        // Setup peer 1
        let mut peer_1 =
            libp2p_impl::SignerSwarm::new(libp2p_impl::SignerSwarmConfig::new(keypair_1))
                .expect("failed to configure libp2p network");
        peer_1.add_listen_endpoint(
            "/ip4/0.0.0.0/tcp/0"
                .parse()
                .expect("failed to parse listen address"),
        );
        let peer_1 = peer_1.start().await.expect("failed to start network 1");

        // Setup peer 2
        let mut peer_2 =
            libp2p_impl::SignerSwarm::new(libp2p_impl::SignerSwarmConfig::new(keypair_2))
                .expect("failed to configure libp2p network");
        peer_2.add_listen_endpoint(
            "/ip4/0.0.0.0/tcp/0"
                .parse()
                .expect("failed to parse listen address"),
        );
        let peer_2 = peer_2.start().await.expect("failed to start network 1");

        // Wait for the network to mesh (i.e. for the peers to discover and connect to each other)
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        testing::network::assert_clients_can_exchange_messages(peer_1, peer_2).await;
    }
}
