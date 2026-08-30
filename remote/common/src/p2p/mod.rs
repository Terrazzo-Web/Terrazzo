//! Shared signaling and WebRTC transport for Terrazzo Gateway peers.

pub mod data_channel_io;
pub mod peer_connection;
pub mod protocol;

pub static GOOGLE_STUN: &str = "stun:stun.l.google.com:19302";
