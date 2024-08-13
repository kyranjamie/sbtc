use clap::Parser;

use libp2p::identity::Keypair;
use signer::network::{grpc_relay, libp2p_impl::LibP2PNetwork};

/// Relay server for sBTC v1.
///
/// This command spins up a relay server at the given host and port.
/// The relay server accepts connections from clients, and forwards
/// inbound messages to all clients.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = String::from("127.0.0.1"), env = "SBTC_RELAY_HOST")]
    host: String,

    #[arg(long, default_value_t = 50042, env = "SBTC_RELAY_PORT")]
    port: u16,

    #[arg(long, default_value_t = true, env = "SBTC_LOG_PRETTY")]
    pretty_logs: bool,
}

impl Args {
    fn address(&self) -> String {
        let Self { host, port, .. } = self;
        format!("{host}:{port}")
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    sbtc::logging::setup_logging("info,signer=debug,relay-server=debug", args.pretty_logs);

    let libp2p_server = LibP2PNetwork::new(Keypair::generate_ed25519()).expect("failed to create libp2p server");

    let server = grpc_relay::RelayServer::new();
    server.serve(args.address()).await.expect("server failed");
}
