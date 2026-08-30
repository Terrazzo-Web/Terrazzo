//! Wire types exchanged through the P2P signaling server.

use std::collections::HashSet;

use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

/// Current signaling protocol version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Maximum accepted SDP length in bytes.
pub const MAX_SDP_LEN: usize = 1024 * 1024;

/// Maximum accepted ICE candidate field length in bytes.
pub const MAX_ICE_FIELD_LEN: usize = 16 * 1024;

/// Maximum accepted peer-facing failure detail length in bytes.
pub const MAX_FAILURE_DETAIL_LEN: usize = 4 * 1024;

/// An unpredictable identifier allocated for one P2P connection attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct P2pConnectionId(Uuid);

impl P2pConnectionId {
    /// Allocates a new connection identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for P2pConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for P2pConnectionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A WebRTC session description sent through signaling.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "sdp", rename_all = "snake_case")]
pub enum SessionDescription {
    /// An SDP offer.
    Offer(String),

    /// An SDP answer.
    Answer(String),
}

impl SessionDescription {
    /// Returns the raw SDP.
    pub fn sdp(&self) -> &str {
        match self {
            Self::Offer(sdp) | Self::Answer(sdp) => sdp,
        }
    }
}

/// The JSON representation of a trickled ICE candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IceCandidate {
    /// Candidate attribute from the SDP.
    pub candidate: String,

    /// Media stream identification tag, when supplied by the peer.
    pub sdp_mid: Option<String>,

    /// Media description index, when supplied by the peer.
    pub sdp_mline_index: Option<u16>,

    /// ICE username fragment, when supplied by the peer.
    pub username_fragment: Option<String>,

    /// STUN or TURN URL that produced a server-reflexive or relay candidate.
    pub url: Option<String>,
}

/// Stable machine-readable signaling failure categories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// A message failed protocol validation.
    InvalidMessage,

    /// The connection identifier is not active.
    UnknownConnection,

    /// The requested server did not register before the deadline.
    ServerOffline,

    /// The peer has too many pending sessions.
    CapacityExceeded,

    /// WebRTC negotiation failed.
    NegotiationFailed,

    /// The peer disconnected during signaling.
    PeerDisconnected,
}

/// Messages relayed over the signaling WebSockets.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalMessage {
    /// Must be the first message sent by either registering peer.
    Hello {
        /// Signaling wire-protocol version.
        protocol_version: u16,
    },

    /// Introduces a newly allocated connection to the registered server.
    Start {
        /// The connection being introduced.
        connection_id: P2pConnectionId,
    },

    /// Carries an SDP offer or answer.
    Description {
        /// The connection being negotiated.
        connection_id: P2pConnectionId,

        /// The offer or answer.
        description: SessionDescription,
    },

    /// Carries one trickled ICE candidate.
    IceCandidate {
        /// The connection being negotiated.
        connection_id: P2pConnectionId,

        /// The candidate to apply.
        candidate: IceCandidate,
    },

    /// Explicitly marks the end of trickled ICE candidates.
    EndOfCandidates {
        /// The connection being negotiated.
        connection_id: P2pConnectionId,
    },

    /// Cancels an in-progress connection.
    Cancel {
        /// The connection to cancel.
        connection_id: P2pConnectionId,
    },

    /// Reports a structured connection failure.
    Failure {
        /// The connection that failed.
        connection_id: P2pConnectionId,

        /// Stable error category.
        code: FailureCode,

        /// Bounded troubleshooting detail.
        detail: String,
    },
}

impl SignalMessage {
    /// Returns the connection identifier carried by a session message.
    pub fn connection_id(&self) -> Option<P2pConnectionId> {
        match self {
            Self::Hello { .. } => None,
            Self::Start { connection_id }
            | Self::Description { connection_id, .. }
            | Self::IceCandidate { connection_id, .. }
            | Self::EndOfCandidates { connection_id }
            | Self::Cancel { connection_id }
            | Self::Failure { connection_id, .. } => Some(*connection_id),
        }
    }

    /// Validates protocol version and bounded string fields.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Hello { protocol_version } if *protocol_version != PROTOCOL_VERSION => Err(
                ValidationError::UnsupportedProtocolVersion(*protocol_version),
            ),
            Self::Description { description, .. } => {
                validate_non_empty_bounded("sdp", description.sdp(), MAX_SDP_LEN)
            }
            Self::IceCandidate { candidate, .. } => candidate.validate(),
            Self::Failure { detail, .. } => {
                validate_bounded("failure detail", detail, MAX_FAILURE_DETAIL_LEN)
            }
            Self::Hello { .. }
            | Self::Start { .. }
            | Self::EndOfCandidates { .. }
            | Self::Cancel { .. } => Ok(()),
        }
    }

    /// Rejects session messages whose ID is not currently known.
    ///
    /// A recipient should insert an accepted [`SignalMessage::Start`] identifier
    /// before calling this method for that message.
    pub fn validate_known_connection(
        &self,
        known_connections: &HashSet<P2pConnectionId>,
    ) -> Result<(), ValidationError> {
        self.validate()?;
        if let Some(connection_id) = self.connection_id()
            && !known_connections.contains(&connection_id)
        {
            return Err(ValidationError::UnknownConnection(connection_id));
        }
        Ok(())
    }
}

impl IceCandidate {
    fn validate(&self) -> Result<(), ValidationError> {
        validate_non_empty_bounded("candidate", &self.candidate, MAX_ICE_FIELD_LEN)?;
        validate_optional_bounded("sdp_mid", self.sdp_mid.as_deref(), MAX_ICE_FIELD_LEN)?;
        validate_optional_bounded(
            "username_fragment",
            self.username_fragment.as_deref(),
            MAX_ICE_FIELD_LEN,
        )?;
        validate_optional_bounded("url", self.url.as_deref(), MAX_ICE_FIELD_LEN)
    }
}

fn validate_optional_bounded(
    field: &'static str,
    value: Option<&str>,
    maximum: usize,
) -> Result<(), ValidationError> {
    match value {
        Some(value) => validate_bounded(field, value, maximum),
        None => Ok(()),
    }
}

fn validate_non_empty_bounded(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::EmptyField(field));
    }
    validate_bounded(field, value, maximum)
}

fn validate_bounded(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), ValidationError> {
    if value.len() > maximum {
        return Err(ValidationError::FieldTooLong {
            field,
            actual: value.len(),
            maximum,
        });
    }
    Ok(())
}

/// A signaling message failed validation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ValidationError {
    /// The peer uses an unsupported wire-protocol version.
    #[error("Unsupported signaling protocol version {0}")]
    UnsupportedProtocolVersion(u16),

    /// A required string field is empty.
    #[error("{0} must not be empty")]
    EmptyField(&'static str),

    /// A string field exceeds its wire limit.
    #[error("{field} is {actual} bytes; maximum is {maximum}")]
    FieldTooLong {
        /// Field name.
        field: &'static str,

        /// Actual byte length.
        actual: usize,

        /// Maximum accepted byte length.
        maximum: usize,
    },

    /// A session message references an inactive connection.
    #[error("Unknown P2P connection {0}")]
    UnknownConnection(P2pConnectionId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_round_trip_is_tagged_and_versioned() {
        let message = SignalMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
        };
        let json = serde_json::to_string(&message).unwrap();
        assert_eq!(r#"{"type":"hello","protocol_version":1}"#, json);
        assert_eq!(message, serde_json::from_str(&json).unwrap());
        assert_eq!(Ok(()), message.validate());
    }

    #[test]
    fn rejects_unsupported_version_and_oversized_fields() {
        assert_eq!(
            Err(ValidationError::UnsupportedProtocolVersion(2)),
            SignalMessage::Hello {
                protocol_version: 2,
            }
            .validate()
        );

        let connection_id = P2pConnectionId::new();
        assert_eq!(
            Err(ValidationError::FieldTooLong {
                field: "sdp",
                actual: MAX_SDP_LEN + 1,
                maximum: MAX_SDP_LEN,
            }),
            SignalMessage::Description {
                connection_id,
                description: SessionDescription::Offer("x".repeat(MAX_SDP_LEN + 1)),
            }
            .validate()
        );
    }

    #[test]
    fn rejects_unknown_connection_id() {
        let connection_id = P2pConnectionId::new();
        let message = SignalMessage::EndOfCandidates { connection_id };
        assert_eq!(
            Err(ValidationError::UnknownConnection(connection_id)),
            message.validate_known_connection(&HashSet::new())
        );

        assert_eq!(
            Ok(()),
            message.validate_known_connection(&HashSet::from([connection_id]))
        );
    }
}
