//! Signer network peer implementation using libp2p

use std::{
    cell::LazyCell,
    collections::{hash_map::DefaultHasher, VecDeque},
    error::Error,
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use futures::{FutureExt, StreamExt};
use libp2p::{
    gossipsub::{self, IdentTopic, PublishError},
    identity::Keypair,
    kad::{self, store::MemoryStore},
    mdns, noise, ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, Swarm, SwarmBuilder, TransportError,
};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::codec::{Decode, Encode};

use super::Msg;

const TOPIC: LazyCell<IdentTopic> = LazyCell::new(|| IdentTopic::new("sbtc-signer"));

/// Errors that can occur when using the libp2p network
#[derive(Debug, thiserror::Error)]
pub enum SignerSwarmError {
    /// LibP2P builder error
    #[error("libp2p builder error: {0}")]
    Builder(String),
    /// LibP2P swarm error
    #[error("libp2p swarm error: {0}")]
    Swarm(String),
    /// An error occurred while publishing (broadcasting) a message
    #[error("libp2p broadcast error: {0}")]
    Publish(#[from] PublishError),
    /// An error occurred while receiving a message
    #[error("libp2p receive error: {0}")]
    Receive(String),
    /// An error occurred while decoding a message
    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    /// A transport error occurred
    #[error("transport error: {0}")]
    Transport(#[from] TransportError<std::io::Error>),
    /// An error occurred while dialing a peer
    #[error("dial error: {0}")]
    Dial(#[from] libp2p::swarm::DialError),
}

/// A libp2p network implementation for the signer
pub struct SignerSwarm {
    swarm: Swarm<SignerBehavior>,
    listen_addrs: Vec<Multiaddr>,
    seed_addrs: Vec<Multiaddr>,
}

/// A handle to the libp2p network
pub struct SignerSwarmHandle {
    swarm: Arc<Mutex<Swarm<SignerBehavior>>>,
    incoming_messages: Arc<Mutex<VecDeque<Msg>>>,
    run_handle: Option<JoinHandle<Result<(), SignerSwarmError>>>,
}

impl SignerSwarmHandle {
    /// Stop the signer swarm
    pub async fn stop(&mut self) {
        if let Some(handle) = &self.run_handle {
            handle.abort();
            self.run_handle = None;
        }
    }

    /// Get the addresses the libp2p swarm is listening on
    pub async fn get_listen_addrs(&self) -> Vec<String> {
        self.swarm
            .lock()
            .await
            .listeners()
            .map(|addr| addr.to_string())
            .collect()
    }

    /// Get the number of connected peers
    pub async fn get_connected_peer_count(&self) -> usize {
        self.swarm.lock().await.connected_peers().count()
    }

    /// Connect to a peer at the specified multiaddress
    pub async fn dial(&self, peer: Multiaddr) -> Result<(), SignerSwarmError> {
        tracing::info!(%peer, "Dialing peer");
        self.swarm.lock().await.dial(peer)?;
        Ok(())
    }

    /// Receive the next message from the libp2p network
    async fn receive_next(&self) -> Result<Msg, SignerSwarmError> {
        loop {
            if let Some(msg) = self.incoming_messages.lock().await.pop_front() {
                return Ok(msg);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Start the signer swarm
    async fn start(&mut self) -> Result<(), SignerSwarmError> {
        let local_peer_id = self.swarm.lock().await.local_peer_id().to_string();
        let swarm = Arc::clone(&self.swarm);
        let incoming_messages = Arc::clone(&self.incoming_messages);

        let run_handle = tokio::spawn(async move {
            let swarm = Arc::clone(&swarm);
            let incoming_messages = Arc::clone(&incoming_messages);

            loop {
                //tracing::info!(%local_peer_id, "Waiting for swarm event");
                let mut swarm = swarm.lock().await;
                if let Some(Some(next)) = swarm.next().now_or_never() {
                    match next {
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Mdns(
                            mdns::Event::Discovered(list),
                        )) => {
                            for (peer_id, _multiaddr) in list {
                                tracing::info!(%local_peer_id, "mDNS discovered a new peer: {peer_id}");
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Mdns(mdns::Event::Expired(
                            list,
                        ))) => {
                            for (peer_id, _multiaddr) in list {
                                tracing::info!(%local_peer_id, "mDNS discover peer has expired: {peer_id}");
                                swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .remove_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            tracing::info!(%local_peer_id, "Local node is listening on {address}");
                        }
                        SwarmEvent::ConnectionEstablished {
                            peer_id,
                            connection_id,
                            endpoint,
                            ..
                        } => {
                            let peer_addr = endpoint.get_remote_address();
                            tracing::info!(%local_peer_id, %peer_id, %connection_id, %peer_addr, "Connection to peer established");
                        }
                        SwarmEvent::ConnectionClosed {
                            peer_id,
                            connection_id,
                            endpoint,
                            ..
                        } => {
                            let peer_addr = endpoint.get_remote_address();
                            tracing::info!(%local_peer_id, %peer_id, %connection_id, %peer_addr, "Connection to peer closed");
                        }
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Ping(ping)) => {
                            tracing::debug!(%local_peer_id, "Ping received from: {}", ping.peer);
                        }
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Gossipsub(
                            gossipsub::Event::Message {
                                propagation_source: peer_id,
                                message_id: id,
                                message,
                            },
                        )) => {
                            tracing::trace!(%local_peer_id,
                                "Got message: '{}' with id: {id} from peer: {peer_id}",
                                String::from_utf8_lossy(&message.data),
                            );

                            if let Some(msg) = Msg::decode(message.data.as_slice()).ok() {
                                incoming_messages.lock().await.push_back(msg);
                            }
                        }
                        _ => {}
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                }
            }
        });

        self.run_handle = Some(run_handle);
        Ok(())
    }
}

/// Define the behaviors of the [`SignerSwarm`] libp2p network.
#[derive(NetworkBehaviour)]
struct SignerBehavior {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
    ping: ping::Behaviour,
}

impl SignerSwarm {
    /// Configure a new signer swarm with the specified keypair.
    pub fn new(keypair: Keypair) -> Result<Self, Box<dyn Error>> {
        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_behaviour(|key| {
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut hasher = DefaultHasher::new();
                    message.data.hash(&mut hasher);
                    gossipsub::MessageId::from(hasher.finish().to_string())
                };

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .message_id_fn(message_id_fn)
                    .build()?;

                Ok(SignerBehavior {
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )?,
                    mdns: mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?,
                    kademlia: kad::Behaviour::new(
                        key.public().to_peer_id(),
                        MemoryStore::new(key.public().to_peer_id()),
                    ),
                    ping: ping::Behaviour::default(),
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm.behaviour_mut().gossipsub.subscribe(&TOPIC)?;

        Ok(SignerSwarm {
            swarm,
            listen_addrs: Vec::new(),
            seed_addrs: Vec::new(),
        })
    }

    /// Start the signer swarm, consuming this instance and returning a
    /// [`SignerSwarmHandle`] that can be used to interact with the swarm.
    pub async fn start(mut self) -> Result<SignerSwarmHandle, SignerSwarmError> {
        let local_peer_id = self.swarm.local_peer_id().to_string();
        tracing::info!(%local_peer_id, "Starting libp2p swarm");

        // Start listening on the specified addresses
        for addr in self.listen_addrs.iter() {
            tracing::info!(%local_peer_id, %addr, "Beginning to listen on address");
            self.swarm
                .listen_on(addr.clone())
                .expect(&format!("Failed to listen on address '{addr:?}'"));
        }

        // Dial the seed addresses
        for addr in self.seed_addrs.iter() {
            let addr = addr.clone();
            tracing::info!(%local_peer_id, %addr, "Dialing seed address");
            self.swarm.dial(addr)?;
        }

        let mut handle = SignerSwarmHandle {
            swarm: Arc::new(Mutex::new(self.swarm)),
            incoming_messages: Arc::new(Mutex::new(VecDeque::new())),
            run_handle: None,
        };

        handle.start().await?;

        Ok(handle)
    }

    /// Add the specified listener address to the libp2p swarm
    pub fn add_listen_endpoint(&mut self, endpoint: Multiaddr) {
        self.listen_addrs.push(endpoint);
    }

    /// Get the local peer ID of the libp2p swarm
    pub fn get_local_peer_id(&self) -> String {
        self.swarm.local_peer_id().to_string()
    }

    /// Adds a seed address to the libp2p swarm. Seeds are peers which will
    /// be connected to on startup to bootstrap the network.
    pub fn add_seed_addr(&mut self, addr: Multiaddr) {
        self.seed_addrs.push(addr);
    }
}

/// Implementation of [`MessageTransfer`] for [`SignerSwarmHandle`], which
/// provides the ability for generic signer code to publish to and receive
/// messages from the libp2p network.
impl super::MessageTransfer for SignerSwarmHandle {
    type Error = SignerSwarmError;

    #[tracing::instrument(skip(self))]
    async fn broadcast(&mut self, msg: super::Msg) -> Result<(), Self::Error> {
        self.swarm
            .lock()
            .await
            .behaviour_mut()
            .gossipsub
            .publish(TOPIC.clone(), msg.encode_to_vec()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn receive(&mut self) -> Result<Msg, Self::Error> {
        self.receive_next().await
    }
}

#[cfg(test)]
mod tests {
    use rand::thread_rng;
    use tracing_subscriber::EnvFilter;

    use super::*;
    use crate::{ecdsa::Signed, network::MessageTransfer};

    #[tokio::test]
    async fn two_clients_should_be_able_to_exchange_messages_given_a_libp2p_network() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        let keypair_1 = Keypair::generate_ed25519();
        let keypair_2 = Keypair::generate_ed25519();

        let mut network_1 = SignerSwarm::new(keypair_1).unwrap();
        let mut network_2 = SignerSwarm::new(keypair_2).unwrap();

        let network_1_tcp_addr = "/ip4/0.0.0.0/tcp/0";
        let network_2_tcp_addr = "/ip4/0.0.0.0/tcp/0";

        network_1.add_listen_endpoint(
            network_1_tcp_addr
                .parse()
                .expect("failed to parse network 1 multiaddr"),
        );

        network_2.add_listen_endpoint(
            network_2_tcp_addr
                .parse()
                .expect("failed to parse network 2 multiaddr "),
        );

        let msg_1 = Signed::random(&mut thread_rng());
        let msg_2 = Signed::random(&mut thread_rng());

        let peer_1_handle = tokio::spawn(async move {
            let mut network_1 = network_1.start().await.expect("network 1 failed to run");

            // Network 2 will send the first message, so we block until it has been
            // received.
            let recv = network_1
                .receive()
                .await
                .expect("network 1 failed to receive message");
            tracing::info!("Network 1 received a message");

            tracing::info!("Network 1 is responding with message 2");
            network_1
                .broadcast(msg_2.clone())
                .await
                .expect("network 1 failed to broadcast message");

            // Return the messages that were sent and received
            (msg_2, recv)
        });

        let peer_2_handle = tokio::spawn(async move {
            let mut network_2 = network_2.start().await.expect("network 2 failed to run");

            // Wait for a peer to be available before trying to publish, otherwise we will
            // get an `InsufficientPeers` error.
            while network_2.get_connected_peer_count().await == 0 {}

            tracing::info!("Network 2 is connected to a peer");
            tokio::time::sleep(Duration::from_secs(1)).await;

            tracing::info!("Network 2 is broadcasting a message");
            network_2
                .broadcast(msg_1.clone())
                .await
                .expect("network 2 failed to broadcast message");

            let recv = network_2
                .receive()
                .await
                .expect("network 2 failed to receive message");
            tracing::info!("Network 2 received a message");

            // Return the messages that were sent and received
            (msg_1, recv)
        });

        // Wait for both network scenarios to complete
        let (peer_1_result, peer_2_result) = tokio::join!(peer_1_handle, peer_2_handle);

        // Unwrap the results
        let peer_1_result = peer_1_result.expect("network 1 failed");
        let peer_2_result = peer_2_result.expect("network 2 failed");

        // Ensure that msg_1 was sent by network 2 and received by network 1
        assert_eq!(peer_1_result.1, peer_2_result.0);
        // Ensure that msg_2 was sent by network 1 and received by network 2
        assert_eq!(peer_2_result.1, peer_1_result.0);
    }
}
