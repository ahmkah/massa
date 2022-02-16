// Copyright (c) 2021 MASSA LABS <info@massa.net>

use massa_time::MassaTime;
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};

pub const CHANNEL_SIZE: usize = 256;

pub const NODE_SEND_CHANNEL_SIZE: usize = 1024;

/// Limit on the number of peers we advertise to others.
#[cfg(not(test))]
pub const MAX_ADVERTISE_LENGTH: u32 = 10000;
#[cfg(test)]
pub const MAX_ADVERTISE_LENGTH: u32 = 10;

/// Maximum message length in bytes
#[cfg(not(test))]
pub const MAX_MESSAGE_SIZE: u32 = 1048576000;
#[cfg(test)]
pub const MAX_MESSAGE_SIZE: u32 = 3145728;

/// Max number of hash in the message AskForBlocks
#[cfg(not(test))]
pub const MAX_ASK_BLOCKS_PER_MESSAGE: u32 = 128;
#[cfg(test)]
pub const MAX_ASK_BLOCKS_PER_MESSAGE: u32 = 3;

/// Max number of operations per message
pub const MAX_OPERATIONS_PER_MESSAGE: u32 = 1024;

/// Max number of endorsements per message
pub const MAX_ENDORSEMENTS_PER_MESSAGE: u32 = 1024;

pub const HANDSHAKE_RANDOMNESS_SIZE_BYTES: usize = 32;

/// Network configuration
#[derive(Debug, Deserialize, Clone)]
pub struct NetworkSettings {
    /// Where to listen for communications.
    pub bind: SocketAddr,
    /// Our own IP if it is routable, else None.
    pub routable_ip: Option<IpAddr>,
    /// Protocol port
    pub protocol_port: u16,
    /// Time interval spent waiting for a response from a peer.
    /// In millis
    pub connect_timeout: MassaTime,
    /// Network_worker will try to connect to available peers every wakeup_interval.
    /// In millis
    pub wakeup_interval: MassaTime,
    /// Path to the file containing initial peers.
    pub initial_peers_file: std::path::PathBuf,
    /// Path to the file containing known peers.
    pub peers_file: std::path::PathBuf,
    /// Path to the file containing our private_key
    pub private_key_file: std::path::PathBuf,
    /// Config for bootstrap connections.
    pub bootstrap_peers_config: PeerTypeConnectionConfig,
    /// Config for whitelist peers.
    pub whitelist_peers_config: PeerTypeConnectionConfig,
    /// Config for standard peers.
    pub standard_peers_config: PeerTypeConnectionConfig,
    /// Limit on the number of in connections per ip.
    pub max_in_connections_per_ip: usize,
    /// Limit on the number of idle peers we remember.
    pub max_idle_peers: usize,
    /// Limit on the number of banned peers we remember.
    pub max_banned_peers: usize,
    /// Peer database is dumped every peers_file_dump_interval in millis
    pub peers_file_dump_interval: MassaTime,
    /// After message_timeout millis we are no longer waiting on handshake message
    pub message_timeout: MassaTime,
    /// Every ask_peer_list_interval in millis we ask every one for its advertisable peers list.
    pub ask_peer_list_interval: MassaTime,
    /// Max wait time for sending a Network or Node event.
    pub max_send_wait: MassaTime,
    /// Time after which we forget a node
    pub ban_timeout: MassaTime,
    /// Timeout Duration when we send a PeerList in handshake
    pub peer_list_send_timeout: MassaTime,
    /// Max number of in connection overflowed managed by the handshake that send a list of peers
    pub max_in_connection_overflow: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PeerTypeConnectionConfig {
    pub max_in: usize,
    pub target_out: usize,
    pub max_out_attempts: usize,
}

#[cfg(test)]
mod tests {
    use crate::NetworkSettings;
    use massa_time::MassaTime;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::PeerTypeConnectionConfig;

    impl Default for NetworkSettings {
        fn default() -> Self {
            NetworkSettings {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                routable_ip: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                protocol_port: 0,
                connect_timeout: MassaTime::from(180_000),
                wakeup_interval: MassaTime::from(10_000),
                peers_file: std::path::PathBuf::new(),
                max_in_connections_per_ip: 2,
                max_idle_peers: 3,
                max_banned_peers: 3,
                peers_file_dump_interval: MassaTime::from(10_000),
                message_timeout: MassaTime::from(5000u64),
                ask_peer_list_interval: MassaTime::from(50000u64),
                private_key_file: std::path::PathBuf::new(),
                max_send_wait: MassaTime::from(100),
                ban_timeout: MassaTime::from(100_000_000),
                initial_peers_file: std::path::PathBuf::new(),
                peer_list_send_timeout: MassaTime::from(500),
                max_in_connection_overflow: 2,
                bootstrap_peers_config: PeerTypeConnectionConfig {
                    target_out: 1,
                    max_out_attempts: 1,
                    max_in: 1,
                },
                whitelist_peers_config: PeerTypeConnectionConfig {
                    max_in: 10,
                    target_out: 10,
                    max_out_attempts: 10,
                },
                standard_peers_config: PeerTypeConnectionConfig {
                    target_out: 10,
                    max_in: 5,
                    max_out_attempts: 15,
                },
            }
        }
    }
}