//! Configuration management for the signer

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use serde::Deserializer;
use std::net::IpAddr;
use std::sync::LazyLock;
use url::Url;

use crate::error::Error;

/// The default signer network listen-on address.
pub const DEFAULT_NETWORK_HOST: &str = "0.0.0.0";
/// The default signer network listen-on port.
pub const DEFAULT_NETWORK_PORT: u16 = 4122;

#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
/// The Stacks network to use.
pub enum NetworkKind {
    /// The mainnet network
    Mainnet,
    /// The testnet network
    Testnet,
}

/// Top-level configuration for the signer
#[derive(Deserialize, Clone, Debug)]
pub struct Settings {
    /// Blocklist client specific config
    pub blocklist_client: BlocklistClientConfig,
    /// Electrum notifier specific config
    pub block_notifier: BlockNotifierConfig,
    /// Network configuration
    pub network: NetworkConfig,
}

/// Signer network configuration
#[derive(Deserialize, Clone, Debug)]
pub struct NetworkConfig {
    /// List of seeds for the P2P network. If empty then the signer will
    /// only use peers discovered via the `sbtc-discovery` smart contract.
    pub seeds: Vec<String>,
    /// The local network interface(s) to listen on. If empty, then
    /// the signer will use [`DEFAULT_NETWORK_HOST`]:[`DEFAULT_NETWORK_PORT] as
    /// the default and listen on both TCP and QUIC protocols.
    pub listen_on: Vec<String>,
}

/// Blocklist client specific config
#[derive(Deserialize, Clone, Debug)]
pub struct BlocklistClientConfig {
    /// Host of the blocklist client
    pub host: String,
    /// Port of the blocklist client
    pub port: u16,
}

/// Electrum notifier specific config
#[derive(Deserialize, Clone, Debug)]
pub struct BlockNotifierConfig {
    /// Electrum server address
    pub server: String,
    /// Retry interval in seconds
    pub retry_interval: u64,
    /// Maximum retry attempts
    pub max_retry_attempts: u32,
    /// Interval for pinging the server in seconds
    pub ping_interval: u64,
    /// Interval for subscribing to block headers in seconds
    pub subscribe_interval: u64,
}

/// Statically configured settings for the signer
pub static SETTINGS: LazyLock<Settings> =
    LazyLock::new(|| Settings::new().expect("Failed to load configuration"));

impl Settings {
    /// Initializing the global config first with default values and then with provided/overwritten environment variables.
    /// The explicit separator with double underscores is needed to correctly parse the nested config structure.
    pub fn new() -> Result<Self, ConfigError> {
        let env = Environment::with_prefix("SIGNER")
            .separator("__")
            .prefix_separator("_");
        let cfg = Config::builder()
            .add_source(File::with_name("./src/config/default"))
            .add_source(env)
            .build()?;

        let settings: Settings = cfg.try_deserialize()?;

        settings.validate()?;

        Ok(settings)
    }

    /// Perform validation on the configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.blocklist_client.host.is_empty() {
            return Err(ConfigError::Message(
                "[blocklist_client] Host cannot be empty".to_string(),
            ));
        }
        if !(1..=65535).contains(&self.blocklist_client.port) {
            return Err(ConfigError::Message(
                "[blocklist_client] Port must be between 1 and 65535".to_string(),
            ));
        }
        if self.block_notifier.server.is_empty() {
            return Err(ConfigError::Message(
                "[block_notifier] Electrum server cannot be empty".to_string(),
            ));
        }

        // Validate [network.listen_on]
        for addr in &self.network.listen_on {
            self.validate_network_peering_addr("network.listen_on", addr)?;
        }

        // Validate [network.seeds]
        for addr in &self.network.seeds {
            self.validate_network_peering_addr("network.seeds", addr)?;
        }

        Ok(())
    }

    /// Validate a network address used by the peering protocol.
    fn validate_network_peering_addr(&self, section: &str, addr: &str) -> Result<(), ConfigError> {
        if addr.is_empty() {
            return Err(ConfigError::Message(format!(
                "[{section}] Address cannot be empty",
            )));
        }

        let url = Url::parse(addr).map_err(|e| {
            ConfigError::Message(format!("[{section}] Error parsing '{addr}': {e}"))
        })?;

        // Host must be present
        if url.host().is_none() {
            return Err(ConfigError::Message(format!(
                "[{section}] Host cannot be empty: '{addr}'"
            )));
        }

        // We only support TCP and QUIC schemes
        if !["tcp", "quic-v1"].contains(&url.scheme()) {
            return Err(ConfigError::Message(format!(
                "[{section}] Only `tcp` and `quic-v1` schemes are supported"
            )));
        }

        // We don't support URL paths
        if url.path() != "/" {
            return Err(ConfigError::Message(format!(
                "[{section}] Paths are not supported: '{}'",
                url.path()
            )));
        }

        Ok(())
    }
}

/// A deserializer for the url::Url type.
fn url_deserializer<'de, D>(deserializer: D) -> Result<url::Url, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

/// A struct for the entries in the signers Config.toml (which is currently
/// located in src/config/default.toml)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StacksSettings {
    /// The configuration entries related to the Stacks node
    pub node: StacksNodeSettings,
}

/// Settings associated with the stacks node that this signer uses for information
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StacksNodeSettings {
    /// TODO(225): We'll want to support specifying multiple Stacks Nodes
    /// endpoints.
    ///
    /// The endpoint to use when making requests to a stacks node.
    #[serde(deserialize_with = "url_deserializer")]
    pub endpoint: url::Url,
    /// This is the start height of the first EPOCH 3.0 block on the stacks
    /// blockchain.
    pub nakamoto_start_height: u64,
}

impl StacksSettings {
    /// Create a new StacksSettings object by reading the relevant entries
    /// in the signer's config.toml. The values there can be overridden by
    /// environment variables.
    ///
    /// # Notes
    ///
    /// The relevant environment variables and the config entries that are
    /// overridden are:
    ///
    /// * SIGNER_STACKS_API_ENDPOINT <-> stacks.api.endpoint
    /// * SIGNER_STACKS_NODE_ENDPOINT <-> stacks.node.endpoint
    ///
    /// Each of these overrides an entry in the signer's `config.toml`
    pub fn new_from_config() -> Result<Self, Error> {
        let source = File::with_name("./src/config/default");
        let env = Environment::with_prefix("SIGNER")
            .prefix_separator("_")
            .separator("_");

        let conf = Config::builder()
            .add_source(source)
            .add_source(env)
            .build()
            .map_err(Error::SignerConfig)?;

        conf.get::<StacksSettings>("stacks")
            .map_err(Error::StacksApiConfig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_toml_loads_with_environment() {
        // The default toml used here specifies http://localhost:20443
        // as the stacks node endpoint.
        let settings = StacksSettings::new_from_config().unwrap();
        let host = settings.node.endpoint.host();
        assert_eq!(host, Some(url::Host::Domain("localhost")));
        assert_eq!(settings.node.endpoint.port(), Some(20443));

        std::env::set_var("SIGNER_STACKS_NODE_ENDPOINT", "http://whatever:1234");

        let settings = StacksSettings::new_from_config().unwrap();
        let host = settings.node.endpoint.host();
        assert_eq!(host, Some(url::Host::Domain("whatever")));
        assert_eq!(settings.node.endpoint.port(), Some(1234));

        std::env::set_var("SIGNER_STACKS_NODE_ENDPOINT", "http://127.0.0.1:5678");

        let settings = StacksSettings::new_from_config().unwrap();
        let ip: std::net::Ipv4Addr = "127.0.0.1".parse().unwrap();
        assert_eq!(settings.node.endpoint.host(), Some(url::Host::Ipv4(ip)));
        assert_eq!(settings.node.endpoint.port(), Some(5678));

        std::env::set_var("SIGNER_STACKS_NODE_ENDPOINT", "http://[::1]:9101");

        let settings = StacksSettings::new_from_config().unwrap();
        let ip: std::net::Ipv6Addr = "::1".parse().unwrap();
        assert_eq!(settings.node.endpoint.host(), Some(url::Host::Ipv6(ip)));
        assert_eq!(settings.node.endpoint.port(), Some(9101));
    }
}
