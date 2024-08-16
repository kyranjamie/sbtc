//! Signer network peer implementation using libp2p

use std::{
    cell::LazyCell,
    collections::{hash_map::DefaultHasher, VecDeque},
    error::Error,
    hash::{Hash, Hasher},
    net::ToSocketAddrs,
    sync::Arc,
    time::Duration,
};

use futures::{FutureExt, StreamExt};
use libp2p::{
    gossipsub::{self, IdentTopic, PublishError},
    identify,
    identity::Keypair,
    kad::{self, store::MemoryStore},
    mdns, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, Swarm, SwarmBuilder, TransportError,
};
use tokio::{sync::Mutex, task::JoinHandle};
use url::Url;

use crate::{
    codec::{Decode, Encode},
    config::DEFAULT_P2P_PORT,
};

use super::Msg;

/// The topic used for signer gossipsub messages
const TOPIC: LazyCell<IdentTopic> = LazyCell::new(|| IdentTopic::new("sbtc-signer"));
/// The current version of this package as reported by Cargo.
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Errors that can occur when using the libp2p network
#[derive(Debug, thiserror::Error)]
pub enum SignerSwarmError {
    /// LibP2P builder error
    #[error("libp2p builder error: {0}")]
    Builder(&'static str),
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
    /// An error occurred while parsing a multiaddr
    #[error("multiaddr error: {0}")]
    ParseMultiAddr(#[from] libp2p::multiaddr::Error),
    /// An error occurred while parsing a URL
    #[error("url parse error: {0}")]
    ParseUrl(#[from] url::ParseError),
}

/// Builder for a new [`SignerSwarm`].
pub struct SignerSwarmBuilder {
    keypair: Keypair,
    listen_on: Vec<Multiaddr>,
    seed_addrs: Vec<Multiaddr>,
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

/// Define the behaviors of the [`SignerSwarm`] libp2p network.
#[derive(NetworkBehaviour)]
struct SignerBehavior {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
    ping: ping::Behaviour,
    relay: relay::Behaviour,
    identify: identify::Behaviour,
}

/// Configuration for a new [`SignerSwarm`]
pub struct SignerSwarmConfig {
    keypair: Keypair,
    listen_on: Vec<Multiaddr>,
}

impl SignerSwarmBuilder {
    /// Create a new [`SignerSwarmBuilder`] with the specified keypair.
    pub fn new(keypair: Keypair) -> Self {
        SignerSwarmBuilder {
            keypair,
            listen_on: Vec::new(),
            seed_addrs: Vec::new(),
        }
    }

    /// Convert a URL to a list of multiaddresses, performing name resolution
    /// if necessary for hostnames.
    fn url_to_multiaddrs(url: &Url) -> Result<Vec<Multiaddr>, SignerSwarmError> {
        let host = url
            .host_str()
            .ok_or(SignerSwarmError::Builder("host cannot be empty"))?;

        if !["tcp", "quic-v1"].contains(&url.scheme()) {
            return Err(SignerSwarmError::Builder(
                "Only `tcp` and `quic-v1` schemes are supported",
            ));
        }

        let port = if let Some(p) = url.port() {
            p
        } else {
            DEFAULT_P2P_PORT
        };

        let mut multiaddrs: Vec<Multiaddr> = Vec::new();

        if let Ok(addrs) = format!("{host}:{port}").to_socket_addrs() {
            for addr in addrs {
                let multiaddr = format!(
                    "/{}/{}/{}/{}",
                    if addr.is_ipv6() { "ip6" } else { "ip4" },
                    addr.ip(),
                    url.scheme(),
                    addr.port()
                )
                .parse()?;
                multiaddrs.push(multiaddr);
            }
        }

        Ok(multiaddrs)
    }

    /// Add a listen address to the configuration
    pub fn add_listen_addr(mut self, addr: &str) -> Result<Self, SignerSwarmError> {
        let url = Url::parse(addr)?;
        if let Ok(multiaddrs) = Self::url_to_multiaddrs(&url) {
            self.listen_on.extend(multiaddrs);
        } else {
            tracing::warn!(%addr, "Failed to parse listen address");
        }
        Ok(self)
    }

    /// Add a seed address to the configuration
    pub fn add_seed_addr(mut self, addr: &str) -> Result<Self, SignerSwarmError> {
        let url = Url::parse(addr)?;
        self.seed_addrs.extend(Self::url_to_multiaddrs(&url)?);
        Ok(self)
    }
}

impl SignerSwarmConfig {
    /// Create a new [`SignerSwarmConfig`] with the specified keypair
    pub fn new(keypair: Keypair) -> Self {
        SignerSwarmConfig { keypair, listen_on: Vec::new() }
    }

    /// Add a listen address to the configuration
    pub fn add_listen_addr(&mut self, addr: &str, port: u16) -> Result<(), SignerSwarmError> {
        self.listen_on
            .push(format!("/ip4/{}/tcp/{}", addr, port).parse()?);
        Ok(())
    }
}

impl SignerSwarm {
    /// Configure a new signer swarm with the specified keypair.
    pub fn new(config: SignerSwarmConfig) -> Result<Self, Box<dyn Error>> {
        let mut swarm = SwarmBuilder::with_existing_identity(config.keypair.clone())
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
                    relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()),
                    identify: identify::Behaviour::new(identify::Config::new(
                        format!("/sbtc-signer/{}", PKG_VERSION),
                        key.public(),
                    )),
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
    pub fn add_listen_endpoint(&mut self, endpoint: Multiaddr) -> &mut Self {
        self.listen_addrs.push(endpoint);
        self
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
            loop {
                let mut swarm = swarm.lock().await;

                // We poll the swarm for events using `now_or_never`, which lets us release
                // the lock and allow other tasks to run while we wait for events (such
                // as publishing/broadcasting new messages).
                if let Some(Some(next)) = swarm.next().now_or_never() {
                    match next {
                        // Identify protocol events. These are used by the relay to
                        // help determine/verify its own address.
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Identify(
                            identify::Event::Received { info, .. },
                        )) => {
                            tracing::info!(%local_peer_id, "Received identify message from peer; adding to confirmed external addresses");
                            swarm.add_external_address(info.observed_addr.clone())
                        }
                        // A multicast-DNS event indicating that a new peer has been discovered.
                        // mDNS can only be used to discover peers on the same local network, so
                        // this will never be raised for WAN peers.
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Mdns(
                            mdns::Event::Discovered(list),
                        )) => {
                            for (peer_id, _multiaddr) in list {
                                tracing::info!(%local_peer_id, %peer_id, "mDNS discovered a new peer");
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        // A multicast-DNS event indicating that a previously discovered peer
                        // has expired. This is raised when the TTL of the autodiscovery has
                        // expired and the peer's address has not been updated.
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Mdns(mdns::Event::Expired(
                            list,
                        ))) => {
                            for (peer_id, _multiaddr) in list {
                                tracing::info!(%local_peer_id, %peer_id, "mDNS discover peer has expired");
                                swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .remove_explicit_peer(&peer_id);
                            }
                        }
                        // Event which is raised by the swarm to indicate that the local peer
                        // is now listening on the specified address.
                        SwarmEvent::NewListenAddr { address, .. } => {
                            tracing::info!(%local_peer_id, %address, "Local node is listening");
                        }
                        // Event which is raised by the swarm when a new connection to a peer
                        // is established.
                        SwarmEvent::ConnectionEstablished {
                            peer_id,
                            connection_id,
                            endpoint,
                            ..
                        } => {
                            let peer_addr = endpoint.get_remote_address();
                            tracing::info!(%local_peer_id, %peer_id, %connection_id, %peer_addr, "Connection to peer established");
                        }
                        // Event which is raised by the swarm when a connection to a peer is closed,
                        // possibly due to an error or the peer disconnecting.
                        SwarmEvent::ConnectionClosed {
                            peer_id,
                            connection_id,
                            endpoint,
                            ..
                        } => {
                            let peer_addr = endpoint.get_remote_address();
                            tracing::info!(%local_peer_id, %peer_id, %connection_id, %peer_addr, "Connection to peer closed");
                        }
                        // Event which is raised by the Ping protocol when a ping is received.
                        SwarmEvent::Behaviour(SignerBehaviorEvent::Ping(ping)) => {
                            tracing::debug!(%local_peer_id, peer_id = %ping.peer, "Ping received");
                        }
                        // Event which is raised by the Gossipsub protocol when a new message
                        // has been received.
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
    use nix::net::if_::InterfaceFlags;
    use rand::thread_rng;
    use tracing_subscriber::EnvFilter;

    use super::*;
    use crate::{ecdsa::Signed, network::MessageTransfer};

    #[tokio::test]
    async fn two_clients_should_be_able_to_exchange_messages_given_a_libp2p_network() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        let config_1 = SignerSwarmConfig {
            keypair: Keypair::generate_ed25519(),
            listen_on: Vec::new(),
        };
        let config_2 = SignerSwarmConfig {
            keypair: Keypair::generate_ed25519(),
            listen_on: Vec::new(),
        };

        let mut network_1 = SignerSwarm::new(config_1).unwrap();
        let mut network_2 = SignerSwarm::new(config_2).unwrap();

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
                .expect("network 1 failed to broadcast message 2");

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

            tracing::info!("Network 2 is broadcasting message 1");
            network_2
                .broadcast(msg_1.clone())
                .await
                .expect("network 2 failed to broadcast message 1");

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

    #[test]
    fn addr_to_multiaddrs_ipv4() {
        let url = Url::parse("tcp://127.0.0.1:1234").unwrap();
        let multiaddrs = SignerSwarmBuilder::url_to_multiaddrs(&url).unwrap();
        assert_eq!(multiaddrs.len(), 1);
        assert_eq!(multiaddrs[0].to_string(), "/ip4/127.0.0.1/tcp/1234");
    }

    #[test]
    fn addr_to_multiaddrs_ipv6() {
        let url = Url::parse("tcp://[::1]:1234").unwrap();
        let multiaddrs = SignerSwarmBuilder::url_to_multiaddrs(&url).unwrap();
        assert_eq!(multiaddrs.len(), 1);
        assert_eq!(multiaddrs[0].to_string(), "/ip6/::1/tcp/1234");
    }

    #[test]
    fn addr_to_multiaddrs_resolve_localhost() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        let local_addrs = nix::ifaddrs::getifaddrs().unwrap();
        let local_addrs = local_addrs
            .filter(|addr| {
                addr.flags
                    .contains(InterfaceFlags::IFF_UP | InterfaceFlags::IFF_LOOPBACK)
            })
            .filter(|addr| addr.address.is_some())
            .map(|addr| addr.address.unwrap().to_string())
            .collect::<Vec<String>>();

        for addr in local_addrs.iter() {
            tracing::info!(%addr, "Found local address");
        }

        let url = Url::parse("tcp://localhost:4122").unwrap();
        let multiaddrs = SignerSwarmBuilder::url_to_multiaddrs(&url).unwrap();
        assert_eq!(multiaddrs.len(), 1);

        assert_eq!("/ip4/127.0.0.1/tcp/4122", multiaddrs[0].to_string());
    }

    #[test]
    fn addr_to_multiaddrs_resolve_google() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        tracing::info!("Resolving google.com");
        let resolved_addrs = ("google.com", 4122)
            .to_socket_addrs()
            .unwrap()
            .collect::<Vec<_>>();
        for addr in resolved_addrs.iter() {
            tracing::info!(%addr, "Resolved address");
        }

        let multiaddrs =
            SignerSwarmBuilder::url_to_multiaddrs(&Url::parse("tcp://google.com:4122").unwrap())
                .unwrap();

        for addr in multiaddrs.iter() {
            tracing::info!(%addr, "Multiaddr");
        }

        for addr in resolved_addrs.iter() {
            assert!(multiaddrs.contains(
                &format!(
                    "/{}/{}/tcp/{}",
                    if addr.is_ipv4() { "ip4" } else { "ip6" },
                    addr.ip(),
                    addr.port()
                )
                .parse()
                .unwrap()
            ));
        }
    }
}
