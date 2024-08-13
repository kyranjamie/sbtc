//! Signer network peer implementation using libp2p

use std::{
    collections::hash_map::DefaultHasher, error::Error, hash::{Hash, Hasher}, time::Duration
};

use futures::StreamExt;
use libp2p::{
    core::transport::ListenerId, gossipsub::{self, PublishError}, identity::Keypair, mdns, noise, ping, swarm::{NetworkBehaviour, SwarmEvent}, tcp, yamux, Multiaddr, Swarm, SwarmBuilder, TransportError
};
use tokio::select;

use crate::codec::{Decode, Encode};

use super::Msg;

/// Errors that can occur when using the libp2p network
#[derive(Debug, thiserror::Error)]
pub enum LibP2PNetworkError {
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
    Dial(#[from] libp2p::swarm::DialError)
}

/// A libp2p network implementation for the signer
pub struct LibP2PNetwork {
    swarm: Swarm<SignerBehavior>,
    listener_id: Option<ListenerId>
}

#[derive(NetworkBehaviour)]
struct SignerBehavior {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    ping: ping::Behaviour
}

impl LibP2PNetwork {
    /// Create a new libp2p network
    pub fn new(keypair: Keypair) -> Result<Self, Box<dyn Error>> {
        let swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default
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

                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config
                )?;

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id()
                )?;

                Ok(SignerBehavior {
                    gossipsub,
                    mdns,
                    ping: ping::Behaviour::default()
                })
            })?
            .build();

        Ok(LibP2PNetwork {
            swarm,
            listener_id: None
        })
    }

    /// Start the libp2p swarm, listening on the specified endpoint
    pub fn listen_on(&mut self, endpoint: Multiaddr) -> Result<(), LibP2PNetworkError> {
        self.listener_id = Some(self.swarm.listen_on(endpoint)?);
        Ok(())
    }

    /// Get the listener ID of the libp2p swarm
    pub fn get_listener_id(&self) -> Option<String> {
        self.listener_id.map(|id| id.to_string())
    }

    /// Get the local peer ID of the libp2p swarm
    pub fn get_local_peer_id(&self) -> String {
        self.swarm.local_peer_id().to_string()
    }

    /// Connect to a peer at the specified multiaddress
    pub fn dial(&mut self, peer: Multiaddr) -> Result<(), LibP2PNetworkError> {
        self.swarm.dial(peer)?;
        Ok(())
    }

    async fn receive_next(&mut self) -> Result<Msg, LibP2PNetworkError> {
        loop {
            select! {
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(SignerBehaviorEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discovered a new peer: {peer_id}");
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            continue;
                        }
                    },
                    SwarmEvent::Behaviour(SignerBehaviorEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discover peer has expired: {peer_id}");
                            self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            continue;
                        }
                    },
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Local node is listening on {address}");
                        continue;
                    },
                    SwarmEvent::Behaviour(SignerBehaviorEvent::Ping(ping)) => {
                        println!("Ping received from: {}", ping.peer);
                        continue;
                    },
                    SwarmEvent::Behaviour(SignerBehaviorEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message,
                    })) => {
                        println!(
                            "Got message: '{}' with id: {id} from peer: {peer_id}",
                            String::from_utf8_lossy(&message.data),
                        );
                        return Ok(Msg::decode(message.data.as_slice())?);
                    },
                    _ => {}
                }
            }
        }
    }
}

impl super::MessageTransfer for LibP2PNetwork {
    type Error = LibP2PNetworkError;

    #[tracing::instrument(skip(self))]
    async fn broadcast(&mut self, msg: super::Msg) -> Result<(), Self::Error> {
        let topic = gossipsub::IdentTopic::new("signer");
        self.swarm.behaviour_mut().gossipsub.publish(
            topic, 
            msg.encode_to_vec()?
        )?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn receive(&mut self) -> Result<Msg, Self::Error> {
        let msg = self.receive_next().await?;
        tracing::debug!("received message");
        Ok(msg)
    }
}